#![allow(non_snake_case)]

use std::mem;

extern crate objc;

pub use objc::Message;

extern crate cocoa;
pub use self::cocoa::base::{selector, nil, YES, id /* class, BOOL */};
pub use self::cocoa::appkit::{NSApp, NSApplication, NSWindow, NSMenu, NSMenuItem,
                              NSRunningApplication, NSImage, NSSquareStatusItemLength,
                              NSVariableStatusItemLength, NSApplicationActivateIgnoringOtherApps};

extern crate libc;
pub use self::libc::c_void;
pub use self::objc::declare::ClassDecl;
pub use self::objc::runtime::{Class, Object, Sel};

extern crate objc_id;
pub use self::objc_id::Id;

mod objc_ext;
use self::objc_ext::{NSStatusBar, NSStatusItem};

extern crate objc_foundation;
pub use self::cocoa::foundation::{NSAutoreleasePool, NSString, NSData, NSSize, NSInteger};
pub use self::objc_foundation::{INSObject, NSObject};

use std::fs::File;
use std::io::Read;

use super::Barfly;

pub enum Icon {
    Name(String),
    Image(Vec<u8>),
}

pub struct OsxBarfly {
    icon: Icon,
    app: id,
    menu: *mut objc::runtime::Object,
    pool: *mut objc::runtime::Object,
}

impl Barfly for OsxBarfly {
    fn new(name: &str) -> Self {
        unsafe {
            OsxBarfly {
                icon: Icon::Name(name.to_owned()),
                app: NSApp(),
                pool: NSAutoreleasePool::new(nil), /* TODO: not sure about the consequences of creating this here */
                menu: NSMenu::new(nil).autorelease(),
            }
        }
    }

    fn set_icon_from_text(&mut self, text: &str) {
        self.icon = Icon::Name(text.to_owned());
    }
    fn set_icon_from_buffer(&mut self, buffer: Vec<u8>) {
        self.icon = Icon::Image(buffer);
    }

    fn set_icon_from_file(&mut self, file: &str) {
        let mut file = File::open(file).unwrap();
        let mut buffer: Vec<u8> = vec![];
        file.read_to_end(&mut buffer).unwrap();
        self.set_icon_from_buffer(buffer);
    }

    fn add_item(&mut self, menuItem: &str, cbs: Box<FnMut() -> ()>) {
        unsafe {
            let cb_obj = Callback::from(cbs);

            let astring = NSString::alloc(nil);
            let no_key = NSString::init_str(astring, ""); // TODO want this eventually

            let astring = NSString::alloc(nil);
            let itemtitle = NSString::init_str(astring, menuItem);
            let action = sel!(call);
            let aitem = NSMenuItem::alloc(nil);
            let item =
                NSMenuItem::initWithTitle_action_keyEquivalent_(aitem, itemtitle, action, no_key);
            let _: () = msg_send![item, setTarget: cb_obj];

            NSMenu::addItem_(self.menu, item);
        }
    }

    fn add_menu_separator(&mut self) {
        unsafe {
            let item = NSMenuItem::separatorItem(nil);
            NSMenu::addItem_(self.menu, item);
        }
    }

    fn set_title_at_index(&mut self, index: i32, title: &str) {
        unsafe {
            let item = NSMenu::itemAtIndex_(self.menu, index as NSInteger);
            let astring = NSString::alloc(nil);
            let itemtitle = NSString::init_str(astring, title);
            msg_send![item, setTitle: itemtitle];
        }
    }

    // TODO: allow user callback
    fn add_quit_item(&mut self, label: &str) {
        unsafe {
            let no_key = NSString::alloc(nil).init_str("");
            let pref_item = NSString::alloc(nil).init_str(label);
            let pref_action = selector("terminate:");
            let menuitem = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
                pref_item,
                pref_action,
                no_key,
            );

            self.menu.addItem_(menuitem);
        }
    }

    fn quit(&mut self) {
        unsafe { msg_send![self.app, terminate] };
    }

    fn display(&mut self) {
        unsafe {
            let app = &mut self.app;
            app.activateIgnoringOtherApps_(YES);

            const ICON_WIDTH: f64 = 18.0;
            const ICON_HEIGHT: f64 = 18.0;

            let item = match self.icon {
                Icon::Name(ref title) => {
                    let item = NSStatusBar::systemStatusBar(nil).statusItemWithLength(
                        NSVariableStatusItemLength,
                    );
                    let title = NSString::alloc(nil).init_str(&title);
                    item.setTitle_(title);
                    item
                }
                Icon::Image(ref buffer) => {
                    use self::cocoa::appkit::NSButton;

                    let item = NSStatusBar::systemStatusBar(nil)
                        .statusItemWithLength(NSSquareStatusItemLength)
                        .autorelease();
                    let nsdata = {
                        NSData::dataWithBytes_length_(
                            nil,
                            buffer.as_slice().as_ptr() as *const c_void,
                            buffer.len() as u64,
                        ).autorelease()
                    };
                    if nsdata == nil {
                        return;
                    }

                    let nsimage = {
                        NSImage::initWithData_(NSImage::alloc(nil), nsdata).autorelease()
                    };
                    if nsimage == nil {
                        return;
                    }
                    let new_size = NSSize::new(ICON_WIDTH, ICON_HEIGHT);
                    msg_send![nsimage, setSize: new_size];
                    // item.setTitle_(nsimage);
                    item.button().setImage_(nsimage);
                    item

                }
            };
            item.setHighlightMode_(YES);
            item.setMenu_(self.menu);

            let current_app = NSRunningApplication::currentApplication(nil);
            current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
            app.run();
        }
    }
}

// this code is pretty much a rip off of
// https://github.com/SSheldon/rust-objc-foundation/blob/master/examples/custom_class.rs

enum Callback {}
unsafe impl Message for Callback {}

// SO.. some explanation is in order here.  We want to allow closure callbacks that
// can modify their environment.  But we can't keep them on the $name object because
// that is really just a stateless proxy for the objc object.  So we store them
// as numeric pointers values in "ivar" fields on that object.  But, if we store a pointer to the
// closure object, we'll run into issues with thin/fat pointer conversions (because
// closure objects are trait objects and thus fat pointers).  So we wrap the closure in
// another boxed object ($cbs_name), which, since it doesn't use traits, is actually a
// regular "thin" pointer, and store THAT pointer in the ivar.  But...so...oy.
struct CallbackState {
    cb: Box<FnMut() -> ()>,
}

impl Callback {
    fn from(cb: Box<FnMut() -> ()>) -> Id<Self> {
        let cbs = CallbackState { cb: cb };
        let bcbs = Box::new(cbs);

        let ptr = Box::into_raw(bcbs);
        let ptr = ptr as *mut c_void as u64;
        println!("{}", ptr);
        let mut oid = <Callback as INSObject>::new();
        (*oid).setptr(ptr);
        oid
    }

    fn setptr(&mut self, uptr: u64) {
        unsafe {
            let obj = &mut *(self as *mut _ as *mut ::objc::runtime::Object);
            println!("setting the ptr: {}", uptr);
            obj.set_ivar("_cbptr", uptr);
        }
    }
}

// TODO: Drop for $name doesn't get called, probably because objc manages the memory and
// releases it for us.  so we leak the boxed callback right now.

impl INSObject for Callback {
    fn class() -> &'static Class {
        let cname = "Callback";

        let mut klass = Class::get(cname);
        if klass.is_none() {
            println!("registering class for {}", cname);
            let superclass = NSObject::class();
            let mut decl = ClassDecl::new(&cname, superclass).unwrap();
            decl.add_ivar::<u64>("_cbptr");

            extern "C" fn barfly_callback_call(this: &Object, _cmd: Sel) {
                println!("callback, getting the pointer");
                unsafe {
                    let pval: u64 = *this.get_ivar("_cbptr");
                    let ptr = pval as *mut c_void;
                    let ptr = ptr as *mut CallbackState;
                    let mut bcbs: Box<CallbackState> = Box::from_raw(ptr);
                    {
                        println!("cb test from cb");
                        (*bcbs.cb)();
                    }
                    mem::forget(bcbs);
                }
            }

            unsafe {
                decl.add_method(
                    sel!(call),
                    barfly_callback_call as extern "C" fn(&Object, Sel),
                );
            }

            decl.register();
            klass = Class::get(cname);
        }
        klass.unwrap()
    }
}
