use crate::{
    definition,
    error::{Error, Result},
    files,
};
use serde_json::{json, Value as J};
use std::{fs, path::Path};
pub struct Example {
    pub id: &'static str,
    pub title: &'static str,
    template: &'static str,
    data: Option<(&'static str, &'static [u8])>,
}
pub fn examples() -> Vec<Example> {
    vec![
        Example {
            id: "grouped-sales",
            title: "区域分组销售",
            template: include_str!("../../../examples/tutorial/grouped-sales.json"),
            data: Some((
                "sales.csv",
                include_bytes!("../../../examples/tutorial/sales.csv"),
            )),
        },
        Example {
            id: "complex-master",
            title: "复杂主格经营分析",
            template: include_str!("../../../examples/tutorial/complex-master-expansion.json"),
            data: Some((
                "complex-master-sales.csv",
                include_bytes!("../../../examples/tutorial/complex-master-sales.csv"),
            )),
        },
        Example {
            id: "multi-column",
            title: "多栏报表",
            template: include_str!("../../../examples/tutorial/multi-column.json"),
            data: Some((
                "sales.csv",
                include_bytes!("../../../examples/tutorial/sales.csv"),
            )),
        },
        Example {
            id: "chart-gallery",
            title: "统计图集",
            template: include_str!("../../../examples/tutorial/chart-gallery.json"),
            data: None,
        },
        Example {
            id: "images",
            title: "图片与背景",
            template: include_str!("../../../examples/tutorial/images.json"),
            data: None,
        },
    ]
}
pub fn list() -> J {
    json!({"items":examples().iter().map(|e|json!({"id":e.id,"name":e.title})).collect::<Vec<_>>()})
}
pub fn write_package(id: &str, out: &Path, manifest: bool) -> Result<J> {
    let e = examples()
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| Error::new(2, "EXAMPLE_UNKNOWN", "catalog", id))?;
    let j: J = serde_json::from_str(e.template)?;
    let normalized = crate::format::normalize(&j)?;
    definition::validate(&normalized, None)?;
    fs::create_dir(out)?;
    let result = (|| {
        files::create(&out.join("template.json"), &files::json_bytes(&normalized)?)?;
        let data = if manifest {
            fs::create_dir(out.join("data"))?;
            out.join("data")
        } else {
            out.to_path_buf()
        };
        if let Some((name, bytes)) = e.data {
            files::create(&data.join(name), bytes)?;
        }
        if manifest {
            files::create(
                &out.join("manifest.json"),
                &files::json_bytes(
                    &json!({"version":1,"id":e.id,"name":e.title,"description":e.title,"tags":["预制报表","经营分析"],"template":"template.json","data_dir":"data"}),
                )?,
            )?;
        }
        Ok(
            json!({"example":id,"out_dir":files::absolute(out)?,"template":files::absolute(&out.join("template.json"))?}),
        )
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(out);
    }
    result
}
pub fn functions(name: Option<&str>, limit: usize, offset: usize) -> Result<J> {
    // Only evaluator branches with regression examples, never parser name tables.
    let entries = [
        ("to", "to(start,end)", "to(1,3)", "整数闭区间；默认纵向扩展"),
        (
            "sum",
            "sum(cells)",
            "sum(A1:A3)",
            "区域聚合；数据集聚合使用 ds.sum(expr[,filter])",
        ),
        ("count", "count(cells)", "count(A1:A3)", "区域计数"),
        ("if", "if(condition,yes,no)", "if(A1>0,1,0)", "条件分支"),
        ("ifn", "ifn(value,fallback)", "ifn(A1,0)", "空值替换"),
        ("int", "int(value)", "int(3.7)", "数值截断"),
        ("seq", "seq(cell)", "seq(A1)", "指定模板格的当前扩展序号"),
        (
            "year",
            "year(date)",
            "year('2026-10-17')",
            "字符串日期提取年",
        ),
        (
            "month",
            "month(date)",
            "month('2026-10-17')",
            "字符串日期提取月",
        ),
        (
            "avg",
            "avg(cells)",
            "avg(A1:A3)",
            "列表或区域平均值；支持 @a 选项",
        ),
        ("max", "max(cells)", "max(A1:A3)", "列表或区域最大值"),
        ("min", "min(cells)", "min(A1:A3)", "列表或区域最小值"),
        ("value", "value()", "value()", "当前格的值，用于属性表达式"),
        (
            "ds.avg",
            "ds.avg(expr)",
            "ds1.avg(amount)",
            "主格作用域内平均值",
        ),
        (
            "ds.max",
            "ds.max(expr)",
            "ds1.max(amount)",
            "主格作用域内最大值",
        ),
        (
            "ds.min",
            "ds.min(expr)",
            "ds1.min(amount)",
            "主格作用域内最小值",
        ),
        ("ds.count", "ds.count()", "ds1.count()", "主格作用域内行数"),
        (
            "ds.group",
            "ds.group(expr;asc|desc)",
            "ds1.group(region;asc)",
            "主格作用域内分组",
        ),
        (
            "ds.select",
            "ds.select(expr[,filter])",
            "ds1.select(amount)",
            "按主格作用域取明细",
        ),
        (
            "ds.sum",
            "ds.sum(expr[,filter])",
            "ds1.sum(amount,amount>100)",
            "主格作用域内过滤聚合",
        ),
    ];
    let items:Vec<_>=entries.into_iter().filter(|e|name.is_none_or(|n|e.0.contains(n))).map(|(name,signature,example,limits)|json!({"name":name,"signature":signature,"example":example,"limitations":limits})).collect();
    files::page(&items, limit, offset)
}
