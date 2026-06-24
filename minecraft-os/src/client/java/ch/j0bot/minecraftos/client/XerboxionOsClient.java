package ch.j0bot.minecraftos.client;

import ch.j0bot.minecraftos.XerboxionOsMod;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;

import java.util.ArrayDeque;
import java.util.Deque;

/**
 * Entrypoint CLIENT : la SURFACE du xerboxion-os dans Minecraft (v0 = HUD stub).
 *
 * <p>V0 : un HudRenderCallback dessine les N derniers events bus = la
 * cubion-fenetre minimale. Flux via un lecteur SSE (SseClient) /events.
 *
 * <p>DECISION : par defaut le client NE parle PAS directement au daemon (auth
 * SSO). En v0 le SSE est OFF par defaut (env XION_SSE_URL pour debug solo).
 * TODO(v1) : relais S2C CustomPayload serveur->client plutot qu'un SSE direct.
 * TODO : HudRenderCallback possiblement deprecie -> HudLayerRegistrationCallback
 * en 1.21.x ; ajuster si compile-error.
 */
public final class XerboxionOsClient implements ClientModInitializer {
	private static final int MAX_LINES = 8;
	private static final Deque<String> RECENT = new ArrayDeque<>();

	@Override
	public void onInitializeClient() {
		HudRenderCallback.EVENT.register((drawContext, tickDelta) -> renderHud(drawContext));

		String sseUrl = System.getenv("XION_SSE_URL");
		if (sseUrl != null && !sseUrl.isBlank()) {
			new SseClient(sseUrl, XerboxionOsClient::pushEvent).start();
			pushEvent("sse: connecte " + sseUrl);
		} else {
			pushEvent("xerboxion-os HUD pret (SSE off ; set XION_SSE_URL)");
		}

		XerboxionOsMod.LOG.info("[{}] init client OK (HUD stub)", XerboxionOsMod.MOD_ID);
	}

	/** Ajoute une ligne au ring-buffer (appele depuis le thread SSE). */
	public static void pushEvent(String line) {
		synchronized (RECENT) {
			RECENT.addLast(line);
			while (RECENT.size() > MAX_LINES) {
				RECENT.removeFirst();
			}
		}
	}

	private static void renderHud(net.minecraft.client.gui.DrawContext ctx) {
		var client = net.minecraft.client.MinecraftClient.getInstance();
		if (client == null || client.options.hudHidden) {
			return;
		}
		var tr = client.textRenderer;
		int y = 4;
		synchronized (RECENT) {
			for (String line : RECENT) {
				ctx.drawTextWithShadow(tr, line, 4, y, 0x33FF99);
				y += tr.fontHeight + 1;
			}
		}
	}
}
