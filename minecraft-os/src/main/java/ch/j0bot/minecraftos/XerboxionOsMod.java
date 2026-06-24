package ch.j0bot.minecraftos;

import net.fabricmc.api.ModInitializer;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Entrypoint COMMUN (main). Detient le XionBridge partage et expose les
 * constantes (MOD_ID, topics). Cablage serveur dans XerboxionOsServer, client
 * dans ch.j0bot.minecraftos.client.XerboxionOsClient.
 *
 * <p>AUCUNE reference a une classe client-only ici (sinon crash serveur dedie).
 */
public final class XerboxionOsMod implements ModInitializer {
	public static final String MOD_ID = "xerboxion-os";
	public static final Logger LOG = LoggerFactory.getLogger(MOD_ID);

	// ==== Topics du bus (cf. mc-adapter / block-bion / contrat tsoin) ====
	public static final String TOPIC_COMMAND = "minecraft.command"; // {player,dim,cmd,t}
	public static final String TOPIC_EVENT = "minecraft.event";     // {player,dim,kind,data,t}
	public static final String TOPIC_BLOCK = "minecraft.block";     // {op,dim,x,y,z,block,player,t}
	public static final String TOPIC_PLAYER = "minecraft.player";   // {player,dim,kind,t}
	public static final String TOPIC_CHAT = "minecraft.chat";       // {player,dim,msg,t}
	public static final String TOPIC_WORLD = "minecraft.world";     // {seed,name,t}
	public static final String TOPIC_VOICE = "voice.frame";         // {player,dim,seq,t,bytes_len}

	private static volatile XionBridge bridge;

	@Override
	public void onInitialize() {
		bridge = new XionBridge();
		LOG.info("[{}] init commun OK -- pont bus pret", MOD_ID);
		// TODO(v1) : charger config/minecraft-os.json ; enregistrer les listeners
		// ICI (pas seulement dans l'entrypoint server) pour couvrir le solo.
	}

	/** Accesseur du pont bus partage (null tant que onInitialize n'a pas tourne). */
	public static XionBridge bridge() {
		return bridge;
	}
}
