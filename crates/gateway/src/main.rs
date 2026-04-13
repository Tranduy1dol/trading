fn main() {
    let addr = "0.0.0.0:9999";
    let journal_path = "journal.dat";
    gateway::reactor::run(addr, journal_path);
}
