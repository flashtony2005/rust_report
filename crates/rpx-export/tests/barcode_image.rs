//! 引擎产出的条码 PNG 必须能过**导出侧**的图片校验。
//!
//! 这是 `barcode` 的跨层判据。`rpx-engine` 自己的单测只验了 PNG 头（签名 / IHDR /
//! IEND）与尺寸，**证明不了 IDAT 能被真解码器解开** —— 而 `validate_image` 内部走
//! `rust_xlsxwriter::Image::new_from_buffer`，会真的解析 PNG、读像素、建图像对象。
//! 它过了才算「这张图能进 xlsx」；不过就说明手写的 stored-deflate 流有问题。
use rpx_engine::barcode::data_uri;

#[test]
fn engine_barcode_png_passes_export_validation() {
    for (text, module, height) in [
        ("N000000021000111-JIJIA-1", 1, 40),
        ("N000000021000111-JIJIA-10", 1, 40),
        ("A", 2, 60),
        ("0123456789", 3, 96),
    ] {
        let uri = data_uri(text, module, height).unwrap();
        rpx_export::validate_image(&uri)
            .unwrap_or_else(|e| panic!("{text} 的条码 PNG 未通过导出校验：{e}"));
    }
}

#[test]
fn empty_and_non_ascii_are_rejected() {
    assert!(data_uri("", 1, 40).is_err(), "空内容应报错");
    assert!(data_uri("华东", 1, 40).is_err(), "Code 128B 编不了中文");
    assert!(data_uri("A", 0, 40).is_err(), "模块宽度 0 应报错");
    assert!(data_uri("A", 1, 0).is_err(), "高度 0 应报错");
}
