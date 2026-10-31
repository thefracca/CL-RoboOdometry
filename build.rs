// Eseguito da Cargo PRIMA di compilare il programma.
// Serve a incorporare l'icona dentro il file .exe (quella che vedi in Esplora risorse).

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "FTC Path HUD");
        res.set("FileDescription", "FTC Path HUD");
        // Se manca il compilatore di risorse l'app si compila lo stesso, solo senza icona nell'.exe
        if let Err(e) = res.compile() {
            println!("cargo:warning=Icona dell'exe non incorporata: {e}");
        }
    }
}
//gang