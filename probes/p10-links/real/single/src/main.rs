fn main() {
    #[cfg(any(feature = "native", feature = "ssr"))]
    single::install();
    #[cfg(feature = "native")]
    for t in single::all() {
        println!("{t}");
    }
}
