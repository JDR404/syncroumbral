fn main() {
    slint_build::compile("ui/app.slint").expect("no se pudo compilar la interfaz Slint");
    // Icono y datos del ejecutable en el Explorador y la barra de tareas.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "SyncroUmbral");
        res.set("FileDescription", "SyncroUmbral");
        if let Err(e) = res.compile() {
            println!("cargo:warning=no se pudo incrustar el icono: {e}");
        }
    }
}
