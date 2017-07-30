#[cfg(target_os = "macos")]
pub mod osx;

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;
pub use objc::Message;

pub trait Barfly {
    fn new(name: &str) -> Self;
    fn set_icon_from_text(&mut self, text: &str);
    fn set_icon_from_buffer(&mut self, buffer: Vec<u8>);
    fn set_icon_from_file(&mut self, path: &str);
    fn add_item(&mut self, menuItem: &str, cbs: Box<FnMut() -> ()>);
    fn add_menu_separator(&mut self);
    fn add_quit_item(&mut self, label: &str);
    fn set_title_at_index(&mut self, index: i32, title: &str);
    fn display(&mut self);
    fn quit(&mut self);
}

#[cfg(target_os = "macos")]
pub type PlatformFly = osx::OsxBarfly;

pub fn new(name: &str) -> PlatformFly {
    PlatformFly::new(name)
}

#[test]
fn it_works() {
    let mut bf = new("Test"); //this is barfly::new()
    bf.add_item("Test", Box::new(|| {}));
}
