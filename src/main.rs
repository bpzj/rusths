use crate::guest::rand_account;

mod guest;

fn main() {
    for _i in 0..1000 {
        println!("{:?}", rand_account());
    }
}
