fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let mut res = winres::WindowsResource::new();
        res.set_icon("app.ico");
        res.set("ProductName", "SC 小工具");
        res.set("FileDescription", "SC 小工具");
        res.set("LegalCopyright", "游侠173");
        res.compile().expect("failed to compile Windows resources");
    }
}
