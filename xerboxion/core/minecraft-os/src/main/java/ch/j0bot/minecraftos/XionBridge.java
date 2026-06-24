package ch.j0bot.minecraftos;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Le PONT vers le bus du xion.
 *
 * <p>Transforme chaque event de jeu en un POST {base}/emit avec un body JSON
 * {"topic":"...","payload":"..."} ou payload est le JSON METIER deja serialise
 * EN CHAINE (double-encodage) -- le contrat que l'hote (serve.rs::EmitBody)
 * injecte verbatim et que les ploxions (mc-adapter, block-bion) consomment.
 *
 * <p>Invariants (lecon MEMORY : ne pas surcharger le VPS) :
 * ASYNC STRICT (handlers = offer() non-bloquant, aucun reseau sur le tick),
 * FAIL-SOFT (timeouts courts, aucune exception ne remonte, xion down = drop),
 * BORNE (file ArrayBlockingQueue, pleine -> drop-oldest).
 *
 * <p>TODO(v1) : circuit-breaker + re-probe GET /healthz toutes les 10s.
 */
public final class XionBridge {
	private static final Logger LOG = LoggerFactory.getLogger("xerboxion-os/bridge");

	/** Defaut = IP INTERNE du daemon. JAMAIS le domaine public ici (auth SSO). */
	private static final String DEFAULT_BASE = "http://10.0.0.1:8730";
	private static final int QUEUE_CAPACITY = 1024;
	private static final Duration CONNECT_TIMEOUT = Duration.ofMillis(500);
	private static final Duration REQUEST_TIMEOUT = Duration.ofSeconds(1);

	private final String base;
	private final HttpClient http;
	private final BlockingQueue<String[]> queue = new ArrayBlockingQueue<>(QUEUE_CAPACITY);
	private final Thread worker;
	private final AtomicBoolean running = new AtomicBoolean(true);
	private final AtomicLong dropped = new AtomicLong();
	private final AtomicLong sent = new AtomicLong();

	public XionBridge() {
		String env = System.getenv("XION_BUS_URL");
		if (env == null || env.isBlank()) {
			env = System.getenv("XION_URL");
		}
		this.base = (env == null || env.isBlank()) ? DEFAULT_BASE : env.trim();

		this.http = HttpClient.newBuilder()
				.connectTimeout(CONNECT_TIMEOUT)
				.version(HttpClient.Version.HTTP_1_1)
				.build();

		this.worker = new Thread(this::drainLoop, "xerboxion-bus-emit");
		this.worker.setDaemon(true);
		this.worker.start();
		LOG.info("XionBridge -> {}/emit (file bornee {}, async fail-soft)", base, QUEUE_CAPACITY);
	}

	/** Enqueue un emit. NON-BLOQUANT, appelable depuis le thread serveur. */
	public void emit(String topic, String payloadJson) {
		if (!running.get()) {
			return;
		}
		String[] item = new String[] { topic, payloadJson };
		if (!queue.offer(item)) {
			queue.poll();
			queue.offer(item);
			long d = dropped.incrementAndGet();
			if (d % 256 == 1) {
				LOG.warn("XionBridge file pleine, {} events droppes", d);
			}
		}
	}

	/** Helper : grave un residu via tsoin.record {name, bytes:<hex>}. TODO(v1). */
	public void tsoinRecord(String name, String hexBytes) {
		emit("tsoin.record",
				"{\"name\":\"" + Json.esc(name) + "\",\"bytes\":\"" + Json.esc(hexBytes) + "\"}");
	}

	private void drainLoop() {
		while (running.get() || !queue.isEmpty()) {
			String[] item;
			try {
				item = queue.poll(500, TimeUnit.MILLISECONDS);
			} catch (InterruptedException e) {
				Thread.currentThread().interrupt();
				break;
			}
			if (item == null) {
				continue;
			}
			post(item[0], item[1]);
		}
	}

	private void post(String topic, String payloadJson) {
		// Body du bus : {topic, payload} ou payload = STRING (le JSON metier echappe).
		String body = "{\"topic\":\"" + Json.esc(topic) + "\",\"payload\":\""
				+ Json.esc(payloadJson) + "\"}";
		try {
			HttpRequest req = HttpRequest.newBuilder()
					.uri(URI.create(base + "/emit"))
					.timeout(REQUEST_TIMEOUT)
					.header("Content-Type", "application/json")
					.POST(HttpRequest.BodyPublishers.ofString(body))
					.build();
			HttpResponse<Void> resp = http.send(req, HttpResponse.BodyHandlers.discarding());
			if (resp.statusCode() == 202 || resp.statusCode() == 200) {
				sent.incrementAndGet();
			} else {
				LOG.debug("emit {} -> HTTP {} (fail-soft, drop)", topic, resp.statusCode());
			}
		} catch (Exception e) {
			LOG.debug("emit {} echoue (fail-soft): {}", topic, e.toString());
		}
	}

	/** A appeler sur SERVER_STOPPING : flush best-effort + ferme le worker. */
	public void shutdown() {
		running.set(false);
		worker.interrupt();
		LOG.info("XionBridge stop (envoyes={}, droppes={})", sent.get(), dropped.get());
	}
}
