//! 图形定义通过 RPX 自定义属性保存，表达式在宿主实例的主格上下文中求值。
use crate::value::Value;

mod calculate;
mod chart;
pub(crate) use calculate::collect;

/// 自有扩展命名空间，不占用产品未知的二进制图形槽。
pub const PREFIX: &str = "rust_report.visual.";
pub const BACKGROUND: &str = "rust_report.page_background";

/// 图形坐标最终以浮点数绘制；十进制原值在数据层保持不变。
pub fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Decimal(s) => s.parse().ok(),
        _ => crate::expr::num::to_f64(v),
    }
}

/// 教程的 17 种基本图与 11 种高级图。
macro_rules! chart_kinds {
    ($($kind:ident => $name:literal),+ $(,)?)=>{
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ChartKind {$($kind),+}
        impl ChartKind {
            pub const ALL: &'static [Self]=&[$(Self::$kind),+];
            pub fn parse(s:&str)->Option<Self>{Some(match s {$($name=>Self::$kind),+, _=>return None})}
            pub fn name(self)->&'static str{match self {$(Self::$kind=>$name),+}}
        }
    }
}
chart_kinds! {
    Column=>"column",Column3d=>"column_3d",ClusteredColumn3d=>"clustered_column_3d",
    StackedColumn=>"stacked_column",StackedColumn3d=>"stacked_column_3d",
    Pie=>"pie",Pie3d=>"pie_3d",Line=>"line",Curve=>"curve",Line3d=>"line_3d",
    Area=>"area",Area3d=>"area_3d",Bar=>"bar",ClusteredBar3d=>"clustered_bar_3d",
    StackedBar=>"stacked_bar",StackedBar3d=>"stacked_bar_3d",Scatter=>"scatter",
    ColumnLine=>"column_line",DualLine=>"dual_line",TimeSeries=>"time_series",
    StateTimeline=>"state_timeline",Gantt=>"gantt",Gauge=>"gauge",Gauge3d=>"gauge_3d",
    Radar=>"radar",Milestone=>"milestone",ErrorBar=>"error_bar",Range=>"range"
}
impl ChartKind {
    pub fn intervals(self) -> bool {
        matches!(self, Self::StateTimeline | Self::Gantt | Self::Milestone)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tasks {
    pub categories: Vec<Value>,
    pub starts: Vec<Value>,
    pub ends: Vec<Value>,
    pub states: Vec<Value>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub colors: Vec<String>,
    pub legend: String,
    pub background: String,
    pub margins: [u32; 4],
    pub pie_rotation: f64,
    pub pie_depth: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub name: String,
    pub values: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    pub tasks: Option<Tasks>,
    pub appearance: Appearance,
    pub kind: ChartKind,
    pub title: String,
    pub categories: Vec<Value>,
    pub series: Vec<Series>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphicContent {
    Chart(Box<Chart>),
    /// PNG/JPEG/GIF/BMP data URI。传递到显示与导出阶段，不读取任意文件路径。
    Image(String),
    CellBackground(String),
    /// 已完成类型化参数插值的 ECharts option JSON；不执行 JavaScript 源码。
    Echarts(String),
    /// 在浏览器独立沙箱内执行的 JavaScript 模板体。
    EchartsScript(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Graphic {
    pub row_span: i32,
    pub col_span: i32,
    pub row: i32,
    pub col: i32,
    pub width: u32,
    pub height: u32,
    pub content: GraphicContent,
}
