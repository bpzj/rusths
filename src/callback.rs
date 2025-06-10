use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::sync::Arc;
use std::sync::Mutex;

pub type CallbackFn = Box<dyn Fn(*const c_char) + Send + 'static>;

// 全局回调存储
lazy_static::lazy_static! {
    static ref CALLBACKS: Arc<Mutex<Vec<CallbackFn>>> = Arc::new(Mutex::new(Vec::new()));
}

// 注册回调函数
pub fn register_callback<F>(callback: F) -> usize 
where
    F: Fn(*const c_char) + Send + 'static,
{
    let mut callbacks = CALLBACKS.lock().unwrap();
    let index = callbacks.len();
    callbacks.push(Box::new(callback));
    index
}

// 取消注册回调函数
pub fn unregister_callback(index: usize) {
    let mut callbacks = CALLBACKS.lock().unwrap();
    if index < callbacks.len() {
        callbacks.remove(index);
    }
}

// 这个函数会被传递给 DLL
#[unsafe(no_mangle)]
pub extern "C" fn callback_handler(data: *const c_char, user_data: *mut c_void) {
    let callbacks = CALLBACKS.lock().unwrap();
    let index = user_data as usize;
    
    if let Some(callback) = callbacks.get(index) {
        callback(data);
    }
}

// 使用示例
pub fn example_usage() {
    // 注册回调函数
    let callback_index = register_callback(|data| {
        if !data.is_null() {
            unsafe {
                if let Ok(s) = CStr::from_ptr(data).to_str() {
                    println!("Received data: {}", s);
                }
            }
        }
    });

    // callback_index 可以作为 user_data 传递给 DLL
    let user_data = callback_index as *mut c_void;

    // 当不再需要回调时
    unregister_callback(callback_index);
}

// 安全的包装器
pub struct CallbackWrapper {
    index: usize,
}

impl CallbackWrapper {
    pub fn new<F>(callback: F) -> Self 
    where
        F: Fn(*const c_char) + Send + 'static,
    {
        let index = register_callback(callback);
        Self { index }
    }

    pub fn get_user_data(&self) -> *mut c_void {
        self.index as *mut c_void
    }
}

impl Drop for CallbackWrapper {
    fn drop(&mut self) {
        unregister_callback(self.index);
    }
} 