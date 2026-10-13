/*
 * Json.java -- minimal dependency-free JSON writer.
 *
 * The generator must stay dependency-free so it can be compiled against the
 * product jars alone; pulling in a JSON library would add a classpath entry
 * that the fixture build has no reason to carry.
 */
package rpx.fixtures;

/** Minimal JSON object builder. Insertion order is preserved. */
final class JsonObj {
	private final StringBuilder sb = new StringBuilder();
	private boolean first = true;

	JsonObj() {
		sb.append('{');
	}

	private void sep() {
		if (!first) {
			sb.append(',');
		}
		first = false;
	}

	JsonObj put(String key, String raw) {
		sep();
		sb.append(Json.quote(key)).append(':').append(raw);
		return this;
	}

	JsonObj put(String key, long v) {
		return put(key, Long.toString(v));
	}

	JsonObj put(String key, double v) {
		return put(key, Double.toString(v));
	}

	JsonObj put(String key, boolean v) {
		return put(key, v ? "true" : "false");
	}

	JsonObj put(String key, JsonObj o) {
		return put(key, o.done());
	}

	JsonObj put(String key, JsonArr a) {
		return put(key, a.done());
	}

	JsonObj putString(String key, String s) {
		return put(key, Json.quote(s));
	}

	String done() {
		return sb.append('}').toString();
	}
}

/** Minimal JSON array builder. Insertion order is preserved. */
final class JsonArr {
	private final StringBuilder sb = new StringBuilder();
	private boolean first = true;

	JsonArr() {
		sb.append('[');
	}

	private void sep() {
		if (!first) {
			sb.append(',');
		}
		first = false;
	}

	void add(String raw) {
		sep();
		sb.append(raw);
	}

	void add(long v) {
		add(Long.toString(v));
	}

	void add(double v) {
		add(Double.toString(v));
	}

	void add(JsonObj o) {
		add(o.done());
	}

	String done() {
		return sb.append(']').toString();
	}
}

/** String escaping only; numbers and booleans are emitted verbatim. */
final class Json {
	private Json() {
	}

	static String quote(String s) {
		if (s == null) {
			return "null";
		}
		StringBuilder b = new StringBuilder(s.length() + 2);
		b.append('"');
		for (int i = 0; i < s.length(); i++) {
			char c = s.charAt(i);
			switch (c) {
				case '"':
					b.append("\\\"");
					break;
				case '\\':
					b.append("\\\\");
					break;
				case '\b':
					b.append("\\b");
					break;
				case '\f':
					b.append("\\f");
					break;
				case '\n':
					b.append("\\n");
					break;
				case '\r':
					b.append("\\r");
					break;
				case '\t':
					b.append("\\t");
					break;
				default:
					if (c < 0x20 || c > 0x7E) {
						b.append(String.format("\\u%04x", (int) c));
					} else {
						b.append(c);
					}
			}
		}
		return b.append('"').toString();
	}
}
