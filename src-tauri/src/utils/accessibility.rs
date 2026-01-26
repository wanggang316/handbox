use accessibility::{AXAttribute, AXUIElement};
use core_foundation::string::CFString;

pub fn get_ax_selected_text() -> Option<String> {
    let system_wide = AXUIElement::system_wide();

    // 1. 获取焦点 UI 元素
    let focused_attr = AXAttribute::new(&CFString::from_static_string("AXFocusedUIElement"));
    let focused_cf = system_wide.attribute(&focused_attr).ok()?;
    let focused_element = focused_cf.downcast_into::<AXUIElement>()?;

    // 2. 获取该元素中被选中的文本
    let selected_attr: AXAttribute<core_foundation::base::CFType> = AXAttribute::new(&CFString::from_static_string("AXSelectedText"));
    let text_cf_type = focused_element.attribute(&selected_attr).ok()?;
    let text_cf = text_cf_type.downcast_into::<CFString>()?;

    let text = text_cf.to_string().trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
