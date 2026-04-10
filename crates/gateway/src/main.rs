fn main() {
    let addr = "0.0.0.0:9999";
    println!("Trading engine starting on {}", addr);
    gateway::reactor::run(addr);
}
