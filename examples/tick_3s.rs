
use rusths::ths::{THS, Adjust, Interval, ThsOption};

fn main() {

    // 创建 THS 实例
    let mut ths = THS::new(Some(ThsOption { 
        username: String::new(),
        password: String::new(),
        lib_ver: "116".parse().unwrap() 
    })).expect("Failed to create THS instance");

    // 连接到服务器
    ths.connect().expect("Failed to connect to server");

    // 获取实时行情
    let market_data = ths.get_super_transaction_data("USHA600000",1749087000,1749090600)
        .expect("Failed to get market data");
    println!("实时行情: {:?}", market_data);

    // 断开连接
    ths.disconnect().expect("Failed to disconnect");
} 