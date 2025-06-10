use std::collections::HashMap;
use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use libloading::{Library};
use std::path::PathBuf;
use std::ffi::{CString, CStr};
use std::os::raw::{c_char, c_int, c_void};
use once_cell::sync::OnceCell;


use crate::constants::{MARKETS, BLOCK_MARKETS};
use crate::error::THSError;
use crate::guest;

/// 静态变量，用于缓存库和函数指针
static LIBRARY: OnceCell<Library> = OnceCell::new();
static CALL_FN: OnceCell<unsafe extern "C" fn(*const c_char, *mut c_char, c_int, *const c_void) -> c_int> = OnceCell::new();

#[derive(Debug, Clone)]
pub struct THS {
    ops: HashMap<String, String>,
    lib: &'static Library,
    login: bool,
    share_instance_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(rename(deserialize = "errInfo"))]
    pub err_info: String,
    pub payload: Payload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payload {
    pub result: Option<Value>,
    pub dict_extra: Option<HashMap<String, Value>>,
}

pub struct Adjust;
impl Adjust {
    pub const FORWARD: &'static str = "forward";
    pub const BACKWARD: &'static str = "backward";
    pub const NONE: &'static str = "";

    pub fn all_types() -> Vec<&'static str> {
        vec![Self::FORWARD, Self::BACKWARD, Self::NONE]
    }
}

pub struct Interval;
impl Interval {
    pub const MIN_1: &'static str = "1m";
    pub const MIN_5: &'static str = "5m";
    pub const MIN_15: &'static str = "15m";
    pub const MIN_30: &'static str = "30m";
    pub const MIN_60: &'static str = "60m";
    pub const MIN_120: &'static str = "120m";
    pub const DAY: &'static str = "day";
    pub const WEEK: &'static str = "week";
    pub const MONTH: &'static str = "month";
    pub const QUARTER: &'static str = "quarter";
    pub const YEAR: &'static str = "year";

    pub fn minute_intervals() -> Vec<&'static str> {
        vec![
            Self::MIN_1,
            Self::MIN_5,
            Self::MIN_15,
            Self::MIN_30,
            Self::MIN_60,
            Self::MIN_120,
        ]
    }

    pub fn day_and_above_intervals() -> Vec<&'static str> {
        vec![
            Self::DAY,
            Self::WEEK,
            Self::MONTH,
            Self::QUARTER,
            Self::YEAR,
        ]
    }

    pub fn all_types() -> Vec<&'static str> {
        let mut all = Self::minute_intervals();
        all.extend(Self::day_and_above_intervals());
        all
    }
}

impl THS {
    pub fn new(ops: Option<HashMap<String, String>>) -> Result<Self, THSError> {
        let mut ops = ops.unwrap_or_default();
        if !ops.contains_key("username") || !ops.contains_key("password") {
            let account = guest::rand_account();
            ops.insert("username".to_string(), account.0);
            ops.insert("password".to_string(), account.1);
        }

        // let lib = Self::load_library()?;
        let default_ver = String::new();
        let lib_ver = ops.get("lib_ver").unwrap_or(&default_ver);
        let lib_path = Self::get_lib_path(lib_ver)?;
        let lib: &Library  = LIBRARY.get_or_init(|| unsafe {Library::new(lib_path).unwrap()});

        
        Ok(Self {
            ops,
            lib,
            login: false,
            share_instance_id: 6666666 + rand::random::<i32>().abs() % 2222222,
        })
    }


    fn get_lib_path(version:&str) -> Result<PathBuf, THSError> {
        if std::env::consts::ARCH == "aarch64" {
            return Err(THSError::UnsupportedPlatform("Apple M系列芯片暂不支持".into()));
        }

        let base_dir = std::env::current_dir()?;
        
        #[cfg(target_os = "linux")]
        let lib_name = format!("hq{}.so", version);
        #[cfg(target_os = "macos")]
        let lib_name = format!("hq{}.dylib", version);
        #[cfg(target_os = "windows")]
        let lib_name = format!("hq{}.dll", version);

        let lib_path = base_dir.join("lib").join(lib_name);
        Ok(lib_path)
    }

    pub fn zip_version(&self) -> i32 {
        2
    }

    pub fn next_share_instance_id(&mut self) -> i32 {
        let id = self.share_instance_id;
        self.share_instance_id += 1;
        id
    }

    /// 这里的 params参数 python 可以支持多类型 数据，但是 rust只能是 String 类型的，所以，如果传入的参数在python中是对象，那么前后就不用加 ""
    /// 如下
    /// py 类型     python              rust      
    /// string     "text"             "\"test\""
    /// dict       {'key':'value'}    "{"key":"value"}"
    /// 泛型版本的 call 方法，支持返回不同类型
    pub fn call<T>(&mut self, method: &str, params: Option<String>, buffer_size: usize) -> Result<T, THSError> 
    where T: serde::de::DeserializeOwned {
        let input = format!(
            r#"{{"method":"{}","params":{}}}"#,
            method,
            params.as_deref().unwrap_or("")
        );

        let input_str = CString::new(input).map_err(|e| THSError::ApiError(format!("无效的输入参数: {}", e)))?;

        let mut output_buffer = vec![0u8; buffer_size];
        let output_ptr = output_buffer.as_mut_ptr() as *mut c_char;

        unsafe {
            // 复杂的写法
            // let call_fn = CALL_FN.get_or_init(|| {
            //     *self.lib.get::<unsafe extern "C" fn(*const c_char, *mut c_char, c_int, *const c_void) -> c_int>
            //     (b"Call")
            //         .map_err(|e| THSError::LibraryError(e.to_string())).unwrap()
            //     }
            // );

            let call_fn = CALL_FN.get_or_init(|| { *self.lib.get(b"Call").unwrap() } );

            let result = call_fn(input_str.as_ptr(), output_ptr, buffer_size as c_int, std::ptr::null());

            match result {
                0 => {
                    let output = CStr::from_ptr(output_ptr).to_str().map_err(|e| THSError::ApiError(format!("输出解码失败: {}", e)))?;
                    if output.len() != 0 {
                        serde_json::from_str::<T>(output).map_err(|e| THSError::ApiError(format!("JSON解析失败: {}", e)))
                    } else {
                        serde_json::from_str::<T>("{\"errInfo\":\"\",\"payload\":{}}").map_err(|e| THSError::ApiError(format!("JSON解析失败: {}", e)))
                    }
                },
                -1 => Err(THSError::ApiError(format!(
                    "缓冲区大小不足,当前大小: {:.2} MB",
                    buffer_size as f64 / (1024.0 * 1024.0)
                ))),
                _ => Err(THSError::ApiError(format!(
                    "错误代码: {}, 未找到方法: {}",
                    result, method
                ))),
            }
        }
    }

    // 为了保持向后兼容性，添加一个专门返回 Response 类型的方法
    // pub fn call_response(&mut self, method: &str, params: Option<String>, buffer_size: usize) -> Result<Response, THSError> {
    //     self.call::<Response>(method, params, buffer_size)
    // }

    pub fn connect(&mut self) -> Result<Response, THSError> {
        for attempt in 0..5 {
            let param = serde_json::to_string(&self.ops).unwrap();
            match self.call::<Response>("connect", Some(param), 10 * 1024) {
                Ok(response) => {
                    if response.err_info.is_empty() {
                        self.login = true;
                        println!("✅ 成功连接到服务器");
                        return Ok(response);
                    } else {
                        println!("❌ 第 {} 次连接尝试失败: {}", attempt + 1, response.err_info);
                    }
                }
                Err(e) => {
                    println!("❌ 连接报错: {}", e);
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(1 << attempt));
        }
        Err(THSError::ApiError("尝试 5 次后连接失败".into()))
    }

    pub fn disconnect(&mut self) -> Result<(), THSError> {
        if self.login {
            self.login = false;
            self.call::<Response>("disconnect", None, 1024)?;
            println!("✅ 已成功断开与行情服务器的连接");
        } else {
            println!("✅ 已经断开连接");
        }
        Ok(())
    }

    pub fn help(&mut self, req: &str) -> Result<String, THSError> {
        let response = self.call::<Response>("help", Some(req.to_string()), 1024)?;
        
        match response.payload.result {
            Some(serde_json::Value::String(s)) => Ok(s),
            Some(serde_json::Value::Object(obj)) => {
                Ok(obj.get("help")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string())
            }
            _ => Ok(String::new())
        }
    }

    fn cmd_query_data(&mut self, req: String, service_key: &str, buffer_size: usize, max_attempts: usize) -> Result<Response, THSError> {
        if !self.login {
            return Err(THSError::ApiError("未登录".into()));
        }

        let mut current_buffer_size = buffer_size;
        let mut attempt = 0;

        while attempt < max_attempts {
            match self.call::<Response>(
                &format!("cmd.query_data.{}", service_key),
                Some(req.clone()),
                current_buffer_size,
            ) {
                Ok(response) => {
                    if !response.err_info.is_empty() {
                        println!("查询数据错误信息: {}", response.err_info);
                    }
                    println!("查询执行: {}, 类型: {}", req, service_key);
                    return Ok(response);
                }
                Err(THSError::ApiError(e)) if e.contains("缓冲区大小不足") => {
                    let current_size_mb = current_buffer_size as f64 / (1024.0 * 1024.0);
                    let new_size_mb = (current_buffer_size * 2) as f64 / (1024.0 * 1024.0);
                    println!(
                        "缓冲区大小不足。当前大小: {:.2} MB, 新的大小: {:.2} MB",
                        current_size_mb, new_size_mb
                    );
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    current_buffer_size *= 2;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }

        Err(THSError::ApiError(format!(
            "达到最大尝试次数，请求: {}, 最终缓冲区大小: {}",
            req, current_buffer_size
        )))
    }

    pub fn klines(
        &mut self,
        ths_code: &str,
        start_time: Option<DateTime<Local>>,
        end_time: Option<DateTime<Local>>,
        adjust: &str,
        interval: &str,
        count: i32,
    ) -> Result<Response, THSError> {
        let ths_code = ths_code.to_uppercase();
        if ths_code.len() != 10 || !MARKETS.iter().any(|&m| ths_code.starts_with(m)) {
            return Err(THSError::InvalidCode(
                "证券代码必须为10个字符，且以 'USHA' 或 'USZA' 开头".into(),
            ));
        }

        if !Adjust::all_types().contains(&adjust) {
            return Err(THSError::ApiError(format!("无效的复权类型: {}", adjust)));
        }

        if !Interval::all_types().contains(&interval) {
            return Err(THSError::ApiError(format!("无效的周期类型: {}", interval)));
        }

        let mut params = serde_json::json!({
            "code": ths_code,
            "adjust": adjust,
            "interval": interval,
        });

        if count > 0 {
            params["count"] = serde_json::json!(count);
        } else {
            if let Some(start) = start_time {
                params["start_time"] = serde_json::json!(start.format("%Y-%m-%d %H:%M:%S").to_string());
            }
            if let Some(end) = end_time {
                params["end_time"] = serde_json::json!(end.format("%Y-%m-%d %H:%M:%S").to_string());
            }
        }

        let mut response = self.call::<Response>("klines", Some(params.to_string()), 1024 * 1024)?;

        // 处理返回数据中的时间字段
        if let Some(serde_json::Value::Array(arr)) = response.payload.result.as_mut() {
            for item in arr {
                if let Some(obj) = item.as_object_mut() {
                    if let Some(time_value) = obj.get("时间") {
                        if Interval::minute_intervals().contains(&interval) {
                            if let Some(time_int) = time_value.as_i64() {
                                let hours = time_int / 10000;
                                let minutes = (time_int % 10000) / 100;
                                let seconds = time_int % 100;
                                let time_str = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
                                obj.insert("时间".to_string(), serde_json::Value::String(time_str));
                            }
                        } else {
                            if let Some(time_str) = time_value.as_str() {
                                if let Ok(dt) = Local.datetime_from_str(time_str, "%Y%m%d") {
                                    obj.insert("时间".to_string(), serde_json::Value::String(dt.format("%Y-%m-%d").to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(response)
    }

    pub fn stock_market_data(&mut self, ths_code: &str) -> Result<Response, THSError> {
        let codes = if ths_code.contains(',') {
            ths_code.split(',').collect::<Vec<_>>()
        } else {
            vec![ths_code]
        };

        for code in &codes {
            let code = code.to_uppercase();
            if code.len() != 10 || !MARKETS.iter().any(|&m| code.starts_with(m)) {
                return Err(THSError::InvalidCode(
                    "证券代码必须为10个字符，且以 'USHA' 或 'USZA' 开头".into(),
                ));
            }
        }

        let markets: std::collections::HashSet<_> = codes.iter().map(|c| &c[..4]).collect();
        if markets.len() > 1 {
            return Err(THSError::ApiError("一次性查询多支股票必须市场代码相同".into()));
        }

        let market = markets.into_iter().next().unwrap();
        let short_codes = codes.iter().map(|c| &c[4..]).collect::<Vec<_>>().join(",");
        
        let data_type = "5,6,8,9,10,12,13,402,19,407,24,30,48,49,69,70,3250,920371,55,199112,264648,1968584,461256,1771976,3475914,3541450,526792,3153,592888,592890";
        
        let req = format!(
            "\"id=200&instance={}&zipversion={}&codelist={}&market={}&datatype={}\"",
            self.next_share_instance_id(),
            self.zip_version(),
            short_codes,
            market,
            data_type
        );

        self.cmd_query_data(req, "fu", 1024 * 1024 * 2, 5)
    }

    pub fn get_block_data(&mut self, block_id: i32) -> Result<Response, THSError> {
        let req = format!(
            "\"id=7&instance={}&zipversion={}&sortbegin=0&sortcount=0&sortorder=D&sortid=55\
            &blockid={:x}&reqflag=blockserve\"",
            self.next_share_instance_id(),
            self.zip_version(),
            block_id
        );
        self.cmd_query_data(req, "bk", 1024 * 1024 * 2, 5)
    }

    pub fn get_block_components(&mut self, link_code: &str) -> Result<Response, THSError> {
        if link_code.is_empty() {
            return Err(THSError::ApiError("必须提供板块代码".into()));
        }

        let req = format!(
            "\"id=7&instance={}&zipversion={}&sortbegin=0&sortcount=0&sortorder=D&sortid=55&linkcode={}\"",
            self.next_share_instance_id(),
            self.zip_version(),
            link_code
        );
        self.cmd_query_data(req, "bk", 1024 * 1024 * 2, 5)
    }

    pub fn block_market_data(&mut self, block_code: &str) -> Result<Response, THSError> {
        let codes = if block_code.contains(',') {
            block_code.split(',').collect::<Vec<_>>()
        } else {
            vec![block_code]
        };

        for code in &codes {
            let code = code.to_uppercase();
            if code.len() != 10 || !BLOCK_MARKETS.iter().any(|&m| code.starts_with(m)) {
                return Err(THSError::InvalidCode("板块代码必须为10个字符".into()));
            }
        }

        let markets: std::collections::HashSet<_> = codes.iter().map(|c| &c[..4]).collect();
        if markets.len() > 1 {
            return Err(THSError::ApiError("一次性查询多支股票必须市场代码相同".into()));
        }

        let market = markets.into_iter().next().unwrap();
        let short_codes = codes.iter().map(|c| &c[4..]).collect::<Vec<_>>().join(",");
        
        let data_type = "55,38,39,13,19,92,90,5,275,276,277";
        
        let req = format!(
            "\"id=200&instance={}&zipversion={}&codelist={}&market={}&datatype={}\"",
            self.next_share_instance_id(),
            self.zip_version(),
            short_codes,
            market,
            data_type
        );

        self.cmd_query_data(req, "fu", 1024 * 1024 * 2, 5)
    }

    pub fn query_ths_industry(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xCE5F)
    }

    pub fn query_ths_concept(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xCE5E)
    }

    pub fn query_ths_index(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xD2)
    }

    pub fn stock_zh_lists(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xE)
    }

    pub fn stock_us_lists(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xDC47)
    }

    pub fn stock_hk_lists(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xB)
    }

    pub fn stock_zh_b_lists(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xF)
    }

    pub fn cbond_lists(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xCE14)
    }

    pub fn fund_etf_lists(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xCFF3)
    }

    pub fn fund_etf_t0_lists(&mut self) -> Result<Response, THSError> {
        self.get_block_data(0xD90C)
    }

    pub fn get_transaction_data(&mut self, ths_code: &str, start: i64, end: i64) -> Result<Response, THSError> {
        let ths_code = ths_code.to_uppercase();
        if ths_code.len() != 10 || !MARKETS.iter().any(|&m| ths_code.starts_with(m)) {
            return Err(THSError::InvalidCode(
                "证券代码必须为10个字符，且以 'USHA' 或 'USZA' 开头".into(),
            ));
        }
        if start >= end {
            return Err(THSError::ApiError("开始时间戳必须小于结束时间戳".into()));
        }

        let data_type = "1,5,10,12,18,49";
        let market = &ths_code[..4];
        let short_code = &ths_code[4..];

        let req = format!(
            "\"id=205&instance={}&zipversion={}&code={}&market={}&start={}&end={}&datatype={}&TraceDetail=0\"",
            self.next_share_instance_id(),
            self.zip_version(),
            short_code,
            market,
            start,
            end,
            data_type
        );

        self.cmd_query_data(req, "zhu", 1024 * 1024 * 2, 5)
    }

    pub fn get_super_transaction_data(&mut self, ths_code: &str, start: i64, end: i64) -> Result<Response, THSError> {
        let ths_code = ths_code.to_uppercase();
        if ths_code.len() != 10 || !MARKETS.iter().any(|&m| ths_code.starts_with(m)) {
            return Err(THSError::InvalidCode(
                "证券代码必须为10个字符，且以 'USHA' 或 'USZA' 开头".into(),
            ));
        }
        if start >= end {
            return Err(THSError::ApiError("开始时间戳必须小于结束时间戳".into()));
        }

        let data_type = concat!(
            "1,5,7,10,12,13,14,18,19,20,21,25,26,27,28,29,31,32,33,34,35,49,",
            "69,70,92,123,125,150,151,152,153,154,155,156,157,45,66,661,102,103,",
            "104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,123,125"
        );

        let market = &ths_code[..4];
        let short_code = &ths_code[4..];

        let req = format!(
            "\"id=205&instance={}&zipversion={}&code={}&market={}&start={}&end={}&datatype={}&TraceDetail=0\"",
            self.next_share_instance_id(),
            self.zip_version(),
            short_code,
            market,
            start,
            end,
            data_type
        );

        self.cmd_query_data(req, "zhu", 1024 * 1024 * 2, 5)
    }

    pub fn get_l2_transaction_data(&mut self, ths_code: &str, start: i64, end: i64) -> Result<Response, THSError> {
        let ths_code = ths_code.to_uppercase();
        if ths_code.len() != 10 || !MARKETS.iter().any(|&m| ths_code.starts_with(m)) {
            return Err(THSError::InvalidCode(
                "证券代码必须为10个字符，且以 'USHA' 或 'USZA' 开头".into(),
            ));
        }
        if start >= end {
            return Err(THSError::ApiError("开始时间戳必须小于结束时间戳".into()));
        }

        let data_type = "5,10,12,13";
        let market = &ths_code[..4];
        let short_code = &ths_code[4..];

        let req = format!(
            "\"id=220&instance={}&zipversion={}&code={}&market={}&start={}&end={}&datatype={}\"",
            self.next_share_instance_id(),
            self.zip_version(),
            short_code,
            market,
            start,
            end,
            data_type
        );

        self.cmd_query_data(req, "zhu", 1024 * 1024 * 2, 5)
    }

    pub fn subscribe_test<F>(&mut self, callback: F) -> Result<Response, THSError>
    where
        F: FnMut(*const c_char) + Send + 'static,
    {
        let callback_box = Box::new(callback);
        let callback_ptr = Box::into_raw(callback_box) as *mut c_void;

        self.call::<Response>("subscribe.test", None, 1024 * 1024)
    }

    pub fn subscribe_tick<F>(&mut self, code: &str, callback: F) -> Result<Response, THSError>
    where
        F: FnMut(*const c_char) + Send + 'static,
    {
        let callback_box = Box::new(callback);
        let callback_ptr = Box::into_raw(callback_box) as *mut c_void;

        self.call::<Response>(
            "subscribe.tick",
            Some(code.to_string()),
            1024 * 1024,
        )
    }

    pub fn subscribe_l2<F>(&mut self, code: &str, callback: F) -> Result<Response, THSError>
    where
        F: FnMut(*const c_char) + Send + 'static,
    {
        let callback_box = Box::new(callback);
        let callback_ptr = Box::into_raw(callback_box) as *mut c_void;

        self.call::<Response>(
            "subscribe.l2",
            Some(code.to_string()),
            1024 * 1024,
        )
    }

    pub fn unsubscribe(&mut self, subscribe_id: &str) -> Result<Response, THSError> {
        self.call::<Response>(
            "unsubscribe",
            Some(subscribe_id.to_string()),
            1024,
        )
    }

    pub fn wencai_base(&mut self, condition: &str) -> Result<Response, THSError> {
        self.call::<Response>(
            "wencai_base",
            Some(condition.to_string()),
            1024 * 1024,
        )
    }

    pub fn wencai_nlp(&mut self, condition: &str) -> Result<Response, THSError> {
        self.call::<Response>(
            "wencai_nlp",
            Some(condition.to_string()),
            1024 * 1024 * 8,
        )
    }

    pub fn order_book_ask(&mut self, ths_code: &str) -> Result<Response, THSError> {
        self.call::<Response>(
            "order_book_ask",
            Some("\"".to_owned() + ths_code +"\""),
            1024 * 1024 * 8,
        )
    }

    pub fn order_book_bid(&mut self, ths_code: &str) -> Result<Response, THSError> {
        self.call::<Response>(
            "order_book_bid",
            Some("\"".to_owned() + ths_code +"\""),
            1024 * 1024 * 8,
        )
    }

    pub fn ipo_today(&mut self) -> Result<Response, THSError> {
        self.call::<Response>("ipo_today", None, 1024)
    }

    pub fn ipo_wait(&mut self) -> Result<Response, THSError> {
        self.call::<Response>("ipo_wait", None, 1024)
    }

    pub fn history_minute_time_data(&mut self, ths_code: &str, date: &str, fields: Option<Vec<&str>>) -> Result<Response, THSError> {
        let ths_code = ths_code.to_uppercase();
        if ths_code.len() != 10 || !MARKETS.iter().any(|&m| ths_code.starts_with(m)) {
            return Err(THSError::InvalidCode(
                "证券代码必须为10个字符，且以 'USHA' 或 'USZA' 开头".into(),
            ));
        }

        let data_type = "1,10,13,19,40";
        let market = &ths_code[..4];
        let short_code = &ths_code[4..];

        let req = format!(
            "\"id=207&instance={}&zipversion={}&code={}&market={}&datatype={}&date={}\"",
            self.next_share_instance_id(),
            self.zip_version(),
            short_code,
            market,
            data_type,
            date
        );

        let mut response = self.cmd_query_data(req, "zhu", 1024 * 1024 * 2, 5)?;

        // 处理返回数据中的时间字段和字段过滤
        if let Some(serde_json::Value::Array(arr)) = response.payload.result.as_mut() {
            let mut filtered_arr = Vec::new();
            for item in arr {
                if let Some(obj) = item.as_object() {
                    if let Some(fields) = &fields {
                        if !fields.iter().all(|&field| obj.contains_key(field)) {
                            continue;
                        }
                    }
                    filtered_arr.push(item.clone());
                }
            }
            response.payload.result = Some(serde_json::Value::Array(filtered_arr));
        }

        Ok(response)
    }
}

impl Drop for THS {
    fn drop(&mut self) {
        if self.login {
            let _ = self.disconnect();
        }
    }
} 