package ch.j0bot.minecraftos.client;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.function.Consumer;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Lecteur SSE minimal du flux GET /events du daemon (stub v0). Lit les lignes
 * data: {topic,payload} et pousse un resume court au HUD. Reconnect backoff.
 *
 * <p>Note auth : xion.j0bot.ch derriere forward-auth SSO ; branche seulement en
 * debug solo (env XION_SSE_URL). TODO(v1) : relais S2C serveur->client.
 */
public final class SseClient {
	private static final Logger LOG = LoggerFactory.getLogger("xerboxion-os/sse");

	private final String url;
	private final Consumer<String> sink;
	private final HttpClient http = HttpClient.newBuilder()
			.connectTimeout(Duration.ofSeconds(3))
			.build();
	private volatile boolean running = true;

	public SseClient(String url, Consumer<String> sink) {
		this.url = url;
		this.sink = sink;
	}

	public void start() {
		Thread t = new Thread(this::loop, "xerboxion-sse");
		t.setDaemon(true);
		t.start();
	}

	public void stop() {
		running = false;
	}

	private void loop() {
		long backoff = 1000;
		while (running) {
			try {
				HttpRequest req = HttpRequest.newBuilder()
						.uri(URI.create(url))
						.header("Accept", "text/event-stream")
						.GET()
						.build();
				HttpResponse<java.io.InputStream> resp =
						http.send(req, HttpResponse.BodyHandlers.ofInputStream());
				try (var reader = new java.io.BufferedReader(
						new java.io.InputStreamReader(resp.body(),
								java.nio.charset.StandardCharsets.UTF_8))) {
					String line;
					backoff = 1000;
					while (running && (line = reader.readLine()) != null) {
						if (line.startsWith("data:")) {
							String data = line.substring(5).trim();
							// TODO : parser proprement {topic,payload} ; v0 = tronque.
							sink.accept(data.length() > 60
									? data.substring(0, 60) + "..." : data);
						}
					}
				}
			} catch (Exception e) {
				LOG.debug("SSE {} deconnecte: {}", url, e.toString());
			}
			if (!running) {
				break;
			}
			try {
				Thread.sleep(backoff);
			} catch (InterruptedException ie) {
				Thread.currentThread().interrupt();
				break;
			}
			backoff = Math.min(backoff * 2, 30000);
		}
	}
}
