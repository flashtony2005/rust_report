package rpx.fixtures;

import com.raqsoft.reportl.model.ReportDefine2;
import com.raqsoft.reportl.usermodel.INormalCell;
import com.raqsoft.reportl.usermodel.IRowCell;

/**
 * head 图的**判别性夹具** —— S1/S2（上 head）与 S3/S4（左 head）。
 *
 * <p>为什么必须有它们：在这之前 10 个夹具的 `topHead` **100% 是 cell00**
 * （所有扩展格都是 18 纵向，而 `_$2` 的首分支只认 19 横向）⇒ 上 head 的两条分支
 * **零覆盖**，实现时无论怎么写都"看起来对"。左 head 虽有反例，但**方向不可判别**
 * （每行只有一个 18 扩展格时，递增与递减结果相同）。
 *
 * <p>四个样本各自回答一个问题：
 * <ul>
 * <li>S1 `_$2` 的 19 分支 + head-of-head 分支能不能命中</li>
 * <li>S2 上 head 的方向：两个 19 扩展格上下相邻 ⇒ 取最近还是最远</li>
 * <li>S3 左 head 的方向：同行两个 18 扩展格 ⇒ 取最近还是最远</li>
 * <li>S4 `_$3` 的第二条路径：连续三个 18 扩展格时会跳过"已是 head"的格</li>
 * </ul>
 *
 * <p>另按 team-lead 要求放入**行类型相邻**样本，用来真机判别
 * `_$3` 不调 / `_$2` 调两次 `sameRowTypeAs` 的不对称。
 *
 * <p>⚠ **第一版用 `-95` 做，判别失败**：`ExtNormalCell.sameRowTypeAs:266-269` 把
 * `-91/-95/-94/-90` 当**同一族**，`-95` vs `-91` 返回 **true** ⇒ 结果不变、判别不出来。
 * <p>⇒ 改用 **`-96`（`IRowCell.TYPE_TITLE_HEADER`）**：它不在同族集合里，
 * `-96` vs `-91` 走 `return type1 == type2` ⇒ **false**。于是构成**正反对照**：
 * <ul>
 * <li>`engine_head_top` 的行 2 = `-96`：`(2,1)` 的 topHead **应当改变**（`_$2` 会查行类型）</li>
 * <li>`engine_head_left` 的行 4 = `-96`：同行左侧 18 扩展格的 leftHead **应当不变**
 *     （`_$3` 压根不查行类型）—— 这条**反证**比正证更能锁死"漏掉 `_$3` 里那次调用"</li>
 * </ul>
 * 不用 `-90`（GROUP_HEADER）：它与扩展格相邻会触发已知的 `SimplePager:261` NPE
 * （format-contract.md 附件 J.5）。
 *
 * <p>单独成文件的原因：`EngineModels.java` 已 249 行，加这两个模型会顶破 300 行门禁；
 * 按 `EngineModelsP0` / `EngineModelsM2` 的同一约定拆出。
 */
final class EngineModelsHead {

	private EngineModelsHead() {
	}

	/**
	 * `engine_head_top` —— S1 + S2（上 head 的 19 分支、head-of-head 分支、递减方向）。
	 *
	 * <p>行布局（每行 4 列，数据集 3 行）：
	 * <pre>
	 * 行 1: (1,1)=19 横向扩展格        (1,2)(1,3) 文字，给 19 留出横向空间
	 * 行 2: (2,1)=普通格   ← S1-a：topHead 应为 (1,1)，命中 19 分支
	 * 行 3: (3,1)=19 横向扩展格        (3,2)(3,3) 文字
	 * 行 4: (4,1)=普通格   ← S1-b：(3,1) 自己已是 head ⇒ 走 head-of-head 分支拿到 (1,1)
	 * 行 5: (5,1)=普通格   ← 19 分支，取最近的 (3,1)
	 * 行 6: (6,1)=普通格   ← S2：topHead 应为 (3,1)【最近】而不是 (1,1)【最远】
	 * </pre>
	 */
	static ReportDefine2 topFixture() {
		ReportDefine2 rd = new ReportDefine2(6, 4);
		EngineModels.lit(rd, 1, 2, "h2");
		EngineModels.lit(rd, 1, 3, "h3");
		EngineModels.lit(rd, 1, 4, "h4");
		// 行 2 = -96（TITLE_HEADER）：与行 1 的 19 扩展格**行类型不同族** ⇒
		// `_$2` 的 sameRowTypeAs 返回 false ⇒ 19 分支不命中（这是正反对照的一半）
		rd.getRowCell(2).setRowType(IRowCell.TYPE_TITLE_HEADER);
		EngineModels.lit(rd, 2, 1, "S1a-负对照96行");
		EngineModels.lit(rd, 3, 2, "g2");
		EngineModels.lit(rd, 3, 3, "g3");
		EngineModels.lit(rd, 3, 4, "g4");
		EngineModels.lit(rd, 4, 1, "S1b-查头的头");
		EngineModels.lit(rd, 5, 1, "S1c-19分支");
		EngineModels.lit(rd, 6, 1, "S2-方向判别");
		for (int r : new int[] { 1, 3 }) {
			INormalCell c = rd.getCell(r, 1);
			c.setExtendMode(INormalCell.EXTEND_HORIZONTAL);
			CalcProbe.setExp(c, "ds1.select(region)");
		}
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_sum.csv");
		return rd;
	}

	/**
	 * `engine_head_left` —— S3 + S4（左 head 的递减方向、第二条路径）+ `-95` 相邻样本。
	 *
	 * <p>行布局（每行 6 列）：
	 * <pre>
	 * 行 1: 全部普通格，但 rowType = -95（TABLE_HEADER）← 与行 2 的扩展格相邻，
	 *       用来真机判别 `_$3` 不调 / `_$2` 调两次 sameRowTypeAs 的不对称
	 * 行 2: (2,1)(2,3)=18 纵向扩展格；(2,2)(2,4)(2,5)(2,6)=普通格
	 *       ⇒ (2,4) 的 leftHead 应为 (2,3)【最近】而不是 (2,1)【最远】 ← S3
	 * 行 3: (3,1)(3,2)(3,3)=18 纵向扩展格（连续三个）；(3,4)=普通格；(3,5)(3,6)=普通格
	 *       ⇒ (3,4) 自己已是 head，(3,5) 的 leftHead 应为 (3,3) ← S4 第二条路径
	 * 行 4、5: 普通格
	 * </pre>
	 */
	static ReportDefine2 leftFixture() {
		ReportDefine2 rd = new ReportDefine2(5, 6);
		for (int r = 1; r <= 5; r++) {
			for (int c = 1; c <= 6; c++) {
				EngineModels.lit(rd, r, c, r + "-" + c);
			}
		}
		// 行 1 = -95 表头行（与行 2 的扩展格相邻）
		rd.getRowCell(1).setRowType(IRowCell.TYPE_TABLE_HEADER);
		for (int c : new int[] { 1, 3 }) {
			INormalCell cell = rd.getCell(2, c);
			cell.setExtendMode(INormalCell.EXTEND_VERTICAL);
			CalcProbe.setExp(cell, "ds1.select(qty)");
		}
		for (int c = 1; c <= 3; c++) {
			INormalCell cell = rd.getCell(3, c);
			cell.setExtendMode(INormalCell.EXTEND_VERTICAL);
			CalcProbe.setExp(cell, "ds1.select(qty)");
		}
		// 行 4 = -96（TITLE_HEADER），同行左侧放两个 18 扩展格 ⇒ **反证**：
		// `_$3` 不查行类型，所以 (4,4)(4,5) 的 leftHead 应当与行 2/3 的形态**完全一致**
		// （最近的 18 扩展格），若实现里误加了 sameRowTypeAs，这里就会与行 2 不同。
		rd.getRowCell(4).setRowType(IRowCell.TYPE_TITLE_HEADER);
		for (int c : new int[] { 1, 3 }) {
			INormalCell cell = rd.getCell(4, c);
			cell.setExtendMode(INormalCell.EXTEND_VERTICAL);
			CalcProbe.setExp(cell, "ds1.select(qty)");
		}
		CalcProbe.attachFileDataSet(rd, "ds1", "engine_sum.csv");
		return rd;
	}

	/** 两个夹具都用 `engine_sum.csv` 那 3 行数据。 */
	static String[][] rows() {
		return EngineModelsP0.sameRows();
	}
}