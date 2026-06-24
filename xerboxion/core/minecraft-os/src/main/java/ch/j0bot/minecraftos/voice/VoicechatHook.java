package ch.j0bot.minecraftos.voice;

import ch.j0bot.minecraftos.XerboxionOsMod;
import ch.j0bot.minecraftos.XionBridge;

import net.fabricmc.loader.api.FabricLoader;

import de.maxhenkel.voicechat.api.VoicechatApi;
import de.maxhenkel.voicechat.api.VoicechatPlugin;
import de.maxhenkel.voicechat.api.events.EventRegistration;
import de.maxhenkel.voicechat.api.events.MicrophonePacketEvent;

import java.util.concurrent.atomic.AtomicLong;

/**
 * Hook MINIMAL Simple Voice Chat (le voice-chat-recorder, v0).
 *
 * <p>Enregistre via le ServiceLoader de SVC
 * (META-INF/services/de.maxhenkel.voicechat.api.VoicechatPlugin). SVC charge ce
 * plugin lui-meme -- PAS d'entrypoint Fabric.
 *
 * <p>V0 : capte MicrophonePacketEvent cote serveur -> emit voice.frame
 * {player,dim,seq,t,bytes_len} (METADATA SEULEMENT). On NE met PAS l'audio Opus
 * sur le bus (cap 64 KiB + debit continu) : le record audio = lane ulterieure.
 *
 * <p>Inerte si SVC absent (compileOnly) : SVC ne le chargera pas. La garde
 * isModLoaded est defensive.
 *
 * <p>TODO(v1) : graver la trame opus via XionBridge.tsoinRecord(voice:player:seq, hex).
 * TODO : verifier getSenderConnection / getPacket / getOpusEncodedData contre
 * voicechat-api 2.1.12 reellement resolue.
 */
public class VoicechatHook implements VoicechatPlugin {
	private final AtomicLong seq = new AtomicLong();

	@Override
	public String getPluginId() {
		return XerboxionOsMod.MOD_ID;
	}

	@Override
	public void initialize(VoicechatApi api) {
		if (!FabricLoader.getInstance().isModLoaded("voicechat")) {
			return;
		}
		XerboxionOsMod.LOG.info("[{}] hook Simple Voice Chat actif (metadata v0)",
				XerboxionOsMod.MOD_ID);
	}

	@Override
	public void registerEvents(EventRegistration registration) {
		registration.registerEvent(MicrophonePacketEvent.class, this::onMicPacket);
	}

	private void onMicPacket(MicrophonePacketEvent event) {
		XionBridge b = XerboxionOsMod.bridge();
		if (b == null) {
			return;
		}
		String player = "?";
		try {
			var conn = event.getSenderConnection();
			if (conn != null && conn.getPlayer() != null) {
				Object mcPlayer = conn.getPlayer().getPlayer();
				if (mcPlayer instanceof net.minecraft.server.network.ServerPlayerEntity sp) {
					player = sp.getGameProfile().getName();
				}
			}
		} catch (Throwable ignored) {
			// fail-soft : metadata player best-effort.
		}

		int len = 0;
		try {
			byte[] opus = event.getPacket().getOpusEncodedData();
			if (opus != null) {
				len = opus.length;
			}
		} catch (Throwable ignored) {
			// fail-soft
		}

		long s = seq.incrementAndGet();
		b.emit(XerboxionOsMod.TOPIC_VOICE,
				"{\"player\":\"" + ch.j0bot.minecraftos.Json.esc(player)
						+ "\",\"dim\":\"overworld\",\"seq\":" + s
						+ ",\"t\":" + System.currentTimeMillis()
						+ ",\"bytes_len\":" + len + "}");
	}
}
