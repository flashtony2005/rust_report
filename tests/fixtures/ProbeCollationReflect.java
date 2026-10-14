package rpx.fixtures;

import com.raqsoft.commonl.ChineseComparator;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.text.RuleBasedCollator;
import java.util.Enumeration;
import java.util.Vector;

/**
 * Reflective dump of the compiled collation tables inside the product's
 * RuleBasedCollator (Java 8, sun.text internals).
 *
 * Output (_collation_internals.txt):
 *   - declared fields of RBCollationTables / their types
 *   - mapping: raw per-char values (contract/expand indices visible)
 *   - contractTable: every first-char's EntryPair list (name + value)
 *   - expandTable: expansion lists
 *
 * Defensive: prints whatever exists instead of assuming exact field names.
 */
public class ProbeCollationReflect {

	public static void main(String[] args) throws Exception {
		String out = args.length > 0 ? args[0] : "_collation_internals.txt";
		RuleBasedCollator c = (RuleBasedCollator) ChineseComparator
				.getInstance();
		PrintWriter w = new PrintWriter(new OutputStreamWriter(
				new FileOutputStream(out), "UTF-8"));

		Object tables = null;
		for (Field f : RuleBasedCollator.class.getDeclaredFields()) {
			f.setAccessible(true);
			w.println("RBC field: " + f.getType().getName() + " " + f.getName());
			if (f.getName().equals("tables")) {
				tables = f.get(c);
			}
		}
		if (tables == null) {
			w.println("!! no 'tables' field found");
			w.close();
			return;
		}
		Class<?> tc = tables.getClass();
		w.println("== RBCollationTables class: " + tc.getName());
		for (Field f : tc.getDeclaredFields()) {
			f.setAccessible(true);
			Object v = f.get(tables);
			w.println("  field: " + f.getType().getName() + " " + f.getName()
					+ (v == null ? " = null" : ""));
		}

		// --- mapping ---
		Object mapping = get(tables, "mapping");
		if (mapping != null) {
			w.println("== mapping class: " + mapping.getClass().getName());
			for (Method m : mapping.getClass().getDeclaredMethods()) {
				w.println("  method: " + m);
			}
			Method elem = null;
			try {
				elem = mapping.getClass().getDeclaredMethod("elementAt",
						int.class);
				elem.setAccessible(true);
			} catch (Throwable t) {
			}
			if (elem != null) {
				// dump only chars whose value != UNMAPPED-ish or interesting
				w.println("== mapping values (non-zero only) ==");
				int cnt = 0;
				for (int cp = 0; cp <= 0xFFFF; cp++) {
					int v = ((Integer) elem.invoke(mapping,
							Integer.valueOf(cp))).intValue();
					if (v != 0 && cp < 0x340) {
						w.println(String.format("  U+%04X -> 0x%08X", cp, v));
						cnt++;
					} else if (v != 0 && cp >= 0x340) {
						cnt++;
					}
				}
				w.println("  (non-zero total: " + cnt + ")");
			}
		}

		// --- contractTable ---
		Object ct = get(tables, "contractTable");
		if (ct instanceof Vector) {
			Vector<?> v = (Vector<?>) ct;
			w.println("== contractTable: " + v.size() + " lists ==");
			for (int i = 0; i < v.size(); i++) {
				Object inner = v.elementAt(i);
				if (!(inner instanceof Vector)) {
					w.println("  [" + i + "] not a Vector: " + inner);
					continue;
				}
				Vector<?> list = (Vector<?>) inner;
				StringBuilder sb = new StringBuilder();
				sb.append("  [").append(i).append("] ");
				for (int j = 0; j < list.size(); j++) {
					Object ep = list.elementAt(j);
					sb.append(entryPair(ep));
					if (j < list.size() - 1) {
						sb.append(" | ");
					}
				}
				w.println(sb.toString());
			}
		} else if (ct != null) {
			w.println("== contractTable type: " + ct.getClass().getName());
		}

		// --- expandTable ---
		Object et = get(tables, "expandTable");
		if (et instanceof Vector) {
			Vector<?> v = (Vector<?>) et;
			w.println("== expandTable: " + v.size() + " entries ==");
			for (int i = 0; i < v.size() && i < 40; i++) {
				w.println("  [" + i + "] " + java.util.Arrays.toString((int[]) v.elementAt(i)));
			}
		} else if (et != null) {
			w.println("== expandTable type: " + et.getClass().getName());
		}

		// --- contractFlags ---
		Object cf = get(tables, "contractFlags");
		w.println("== contractFlags: " + (cf == null ? "null" : cf.getClass().getName()));
		if (cf != null) {
			try {
				Field keys = cf.getClass().getDeclaredField("keys");
				keys.setAccessible(true);
				int[] k = (int[]) keys.get(cf);
				StringBuilder sb = new StringBuilder("  keys: ");
				for (int x : k) {
					sb.append(String.format("%04X ", x));
				}
				w.println(sb.toString());
			} catch (Throwable t) {
				w.println("  contractFlags dump failed: " + t);
			}
		}

		w.close();
		System.out.println("internals dumped to " + out);
	}

	private static Object get(Object o, String name) {
		Class<?> k = o.getClass();
		while (k != null) {
			try {
				Field f = k.getDeclaredField(name);
				f.setAccessible(true);
				return f.get(o);
			} catch (Throwable t) {
				k = k.getSuperclass();
			}
		}
		return null;
	}

	private static String entryPair(Object ep) {
		if (ep == null) {
			return "null";
		}
		StringBuilder sb = new StringBuilder();
		sb.append(ep.getClass().getSimpleName()).append("{");
		boolean first = true;
		Class<?> k = ep.getClass();
		while (k != null) {
			for (Field f : k.getDeclaredFields()) {
				f.setAccessible(true);
				try {
					Object v = f.get(ep);
					if (!first) {
						sb.append(", ");
					}
					first = false;
					if (v instanceof String) {
						sb.append(f.getName()).append("=\"").append(cps((String) v))
								.append("\"");
					} else {
						sb.append(f.getName()).append("=").append(v);
					}
				} catch (Throwable t) {
					// ignore
				}
			}
			k = k.getSuperclass();
		}
		sb.append("}");
		return sb.toString();
	}

	private static String cps(String s) {
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < s.length(); i++) {
			if (i > 0) {
				sb.append(' ');
			}
			sb.append(String.format("%04X", (int) s.charAt(i)));
		}
		return sb.toString();
	}
}
