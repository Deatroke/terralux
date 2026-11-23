// Ícone e metadados do .exe no Windows.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/terralux.ico");
        res.set("ProductName", "Terralux Studio");
        res.set("FileDescription", "Terralux Studio");
        if let Err(e) = res.compile() {
            println!("cargo:warning=não consegui embutir o ícone: {e}");
        }
    }
}
