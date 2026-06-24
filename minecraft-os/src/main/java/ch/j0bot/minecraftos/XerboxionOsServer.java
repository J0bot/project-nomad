package ch.j0bot.minecraftos;

import net.fabricmc.api.DedicatedServerModInitializer;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents;
import net.fabricmc.fabric.api.message.v1.ServerMessageEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;

/**
 * Entrypoint SERVEUR : cable les events Fabric du monde vers XionBridge.
 *
 * <p>Monde = generateur(seed)+residu : on n'emet QUE le residu (actions joueur),
 * jamais un dump de chunks (adressage genuine). La seed (emise 1x au boot) +
 * l'algo worldgen reconstruit le reste.
 *
 * <p>Compat FIGEE mc-adapter : les commandes restent sur minecraft.command
 * {player,dim,cmd,t}. On NE re-emet PAS mc.event ni le tsoin commande ici.
 *
 * <p>TODO : capter la POSE de bloc (UseBlockCallback / mixin setBlockState filtre
 * origine joueur) -- non cable en v0 pour eviter le flood worldgen. La CASSE est
 * captee via PlayerBlockBreakEvents.
 * TODO : ServerTickEvents throttle ~1Hz pour minecraft.pos.
 * TODO : capter la commande joueur -> emit minecraft.command (mc-adapter).
 */
public final class XerboxionOsServer implements DedicatedServerModInitializer {

	@Override
	public void onInitializeServer() {
		registerListeners();
	}

	/**
	 * NB : DedicatedServerModInitializer ne tourne QUE en serveur dedie.
	 * TODO(v1) : appeler ceci depuis l'entrypoint main pour le serveur integre solo.
	 */
	public static void registerListeners() {
		final long bootT = System.currentTimeMillis();

		ServerLifecycleEvents.SERVER_STARTED.register(server -> {
			XionBridge b = XerboxionOsMod.bridge();
			if (b == null) {
				return;
			}
			var overworld = server.getOverworld();
			long seed = overworld != null ? overworld.getSeed() : 0L;
			String name = server.getSaveProperties().getLevelName();
			b.emit(XerboxionOsMod.TOPIC_WORLD,
					"{\"seed\":" + seed + ",\"name\":\"" + Json.esc(name)
							+ "\",\"t\":" + System.currentTimeMillis() + "}");
		});

		ServerLifecycleEvents.SERVER_STOPPING.register(server -> {
			XionBridge b = XerboxionOsMod.bridge();
			if (b != null) {
				b.shutdown();
			}
		});

		ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> {
			XionBridge b = XerboxionOsMod.bridge();
			if (b == null) {
				return;
			}
			String player = handler.getPlayer().getGameProfile().getName();
			String dim = dimPath(handler.getPlayer().getWorld());
			b.emit(XerboxionOsMod.TOPIC_PLAYER,
					"{\"player\":\"" + Json.esc(player) + "\",\"dim\":\"" + Json.esc(dim)
							+ "\",\"kind\":\"join\",\"t\":" + System.currentTimeMillis() + "}");
		});
		ServerPlayConnectionEvents.DISCONNECT.register((handler, server) -> {
			XionBridge b = XerboxionOsMod.bridge();
			if (b == null) {
				return;
			}
			String player = handler.getPlayer().getGameProfile().getName();
			String dim = dimPath(handler.getPlayer().getWorld());
			b.emit(XerboxionOsMod.TOPIC_PLAYER,
					"{\"player\":\"" + Json.esc(player) + "\",\"dim\":\"" + Json.esc(dim)
							+ "\",\"kind\":\"quit\",\"t\":" + System.currentTimeMillis() + "}");
		});

		ServerMessageEvents.CHAT_MESSAGE.register((message, sender, params) -> {
			XionBridge b = XerboxionOsMod.bridge();
			if (b == null) {
				return;
			}
			String player = sender.getGameProfile().getName();
			String dim = dimPath(sender.getWorld());
			String msg = message.getContent().getString();
			b.emit(XerboxionOsMod.TOPIC_CHAT,
					"{\"player\":\"" + Json.esc(player) + "\",\"dim\":\"" + Json.esc(dim)
							+ "\",\"msg\":\"" + Json.esc(msg) + "\",\"t\":"
							+ System.currentTimeMillis() + "}");
		});

		PlayerBlockBreakEvents.AFTER.register((world, player, pos, state, entity) -> {
			XionBridge b = XerboxionOsMod.bridge();
			if (b == null) {
				return;
			}
			String block = blockPath(state);
			String dim = dimPath(world);
			String p = player.getGameProfile().getName();
			// block.break {block,dim,x,y,z} consomme par block_ploxion! sans modif.
			b.emit("block.break",
					"{\"block\":\"" + Json.esc(block) + "\",\"dim\":\"" + Json.esc(dim)
							+ "\",\"x\":" + pos.getX() + ",\"y\":" + pos.getY()
							+ ",\"z\":" + pos.getZ() + "}");
			b.emit(XerboxionOsMod.TOPIC_BLOCK,
					"{\"op\":\"break\",\"dim\":\"" + Json.esc(dim) + "\",\"x\":"
							+ pos.getX() + ",\"y\":" + pos.getY() + ",\"z\":" + pos.getZ()
							+ ",\"block\":\"" + Json.esc(block) + "\",\"player\":\""
							+ Json.esc(p) + "\",\"t\":" + System.currentTimeMillis() + "}");
		});

		XerboxionOsMod.LOG.info("[{}] listeners serveur cables (boot {})",
				XerboxionOsMod.MOD_ID, bootT);
	}

	/**
	 * Chemin dimension sans namespace (overworld / the_nether / the_end). Helper
	 * isole = limite le cout de migration mojmap apres 1.21.11.
	 * TODO : normaliser the_nether->nether, the_end->end si besoin consommateur.
	 */
	private static String dimPath(net.minecraft.world.World world) {
		return world.getRegistryKey().getValue().getPath();
	}

	/** Chemin du bloc SANS namespace (ex 'stone') pour matcher BlockDef.id du sdk. */
	private static String blockPath(net.minecraft.block.BlockState state) {
		return net.minecraft.registry.Registries.BLOCK.getId(state.getBlock()).getPath();
	}
}
