//! La ventana principal y el teclado: que las teclas lleguen siempre a la app y que WebView2 no
//! haga cosas de navegador (recargar, buscar, imprimir, zoom…).

use std::sync::atomic::{AtomicU32, Ordering};

use pos_core::servicios::config::EscalaInterfaz;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, Webview, WebviewWindow, WindowEvent};

/// El zoom vigente en milésimas (1100 = 110 %); 0 mientras no se haya leído la configuración.
static ZOOM_MILESIMAS: AtomicU32 = AtomicU32::new(0);

fn poner_zoom(webview: &Webview, milesimas: u32) {
    if let Err(e) = webview.set_zoom(f64::from(milesimas) / 1000.0) {
        tracing::warn!("no se pudo aplicar el tamaño de la interfaz: {e}");
    }
}

/// Tamaño de letra y botones de toda la interfaz: el zoom nativo del webview. Es una sola operación
/// de WebView2 (no corre nada en el hilo de la interfaz) y escala textos, botones y tablas por igual.
/// Se aplica al arrancar, apenas se lee la configuración, y al cambiarlo, sin reiniciar.
pub fn aplicar_escala(ventana: &WebviewWindow, escala: EscalaInterfaz) {
    let milesimas = (escala.factor() * 1000.0).round() as u32;
    ZOOM_MILESIMAS.store(milesimas, Ordering::Relaxed);
    poner_zoom(ventana.as_ref(), milesimas);
}

/// La ventana nace oculta (`"visible": false` en tauri.conf.json): la página empieza a cargar antes de
/// que `setup` termine de abrir la base y leer el tamaño de la interfaz, así que al mostrarla de
/// inmediato se veía todo al 100 % y a los ~300 ms saltaba al tamaño elegido. Se muestra recién cuando
/// la escala ya está aplicada y la página terminó de cargar.
pub fn al_cargar_pagina(webview: &Webview, evento: PageLoadEvent) {
    let milesimas = ZOOM_MILESIMAS.load(Ordering::Relaxed);
    if milesimas != 0 {
        poner_zoom(webview, milesimas);
    }
    if evento == PageLoadEvent::Finished {
        if let Some(ventana) = webview.app_handle().get_webview_window("main") {
            mostrar(&ventana);
        }
    }
}

fn mostrar(ventana: &WebviewWindow) {
    if ventana.is_visible().unwrap_or(true) {
        return;
    }
    let _ = ventana.show();
    let _ = ventana.set_focus();
}

/// Si la página no llega a cargar (un error del front, por ejemplo), la ventana se muestra igual:
/// una app invisible sería peor que una con un error a la vista.
const ESPERA_MAXIMA_PARA_MOSTRAR: std::time::Duration = std::time::Duration::from_secs(8);

/// Se llama una vez en `setup`, con la ventana ya creada.
pub fn preparar(app: &tauri::App) {
    let Some(ventana) = app.get_webview_window("main") else {
        tracing::warn!("no está la ventana principal: no se ajusta el teclado");
        return;
    };
    devolver_foco_al_volver(&ventana);
    let respaldo = ventana.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(ESPERA_MAXIMA_PARA_MOSTRAR).await;
        mostrar(&respaldo);
    });
    #[cfg(windows)]
    sin_teclas_del_navegador(&ventana);
}

/// Al volver a la ventana (Alt+Tab, después de un diálogo nativo de archivo o de imprimir) WebView2
/// a veces no recupera el foco del teclado y las teclas no llegan hasta hacer clic. Aquí se le
/// devuelve el foco al webview.
///
/// Cuidado con el ciclo: `Webview::set_focus` (MoveFocus de WebView2) llega por la cola de eventos
/// y, si el webview ya tiene el foco, WebView2 lo hace rebotar por la ventana principal. Eso
/// produce otro `Focused(true)` (y wry vuelve a llamar a MoveFocus en WM_SETFOCUS). Sin freno, se
/// medía un ciclo de ~800 eventos de foco por segundo. Por eso, después de devolver el foco se
/// ignoran los `Focused(true)` durante un momento: son el rebote, no el usuario volviendo a la
/// ventana (eso tarda mucho más). No activa ni trae la ventana al frente, así que no parpadea.
fn devolver_foco_al_volver(ventana: &WebviewWindow) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    const PAUSA: Duration = Duration::from_millis(800);
    let webview = ventana.as_ref().clone();
    let ultima: Mutex<Option<Instant>> = Mutex::new(None);
    ventana.on_window_event(move |evento| {
        let WindowEvent::Focused(true) = evento else {
            return;
        };
        let Ok(mut ultima) = ultima.lock() else {
            return;
        };
        if ultima.is_some_and(|t| t.elapsed() < PAUSA) {
            return;
        }
        *ultima = Some(Instant::now());
        if let Err(e) = webview.set_focus() {
            tracing::debug!("no se pudo devolver el foco al webview: {e}");
        }
    });
}

/// Desactiva las teclas propias del navegador de WebView2: F5 y Ctrl+R (recargar), F3 y Ctrl+F
/// (buscar), Ctrl+P (imprimir la página), Ctrl+más/menos (zoom), F7 (exploración con el cursor),
/// Atrás y Adelante… Las teclas de edición (copiar, pegar, cortar, deshacer, seleccionar todo)
/// siguen funcionando en los campos, y todas las teclas siguen llegando a la página, donde las
/// atiende el despachador de atajos (src/atajos.ts).
///
/// Se hace de dos formas: el ajuste `AreBrowserAcceleratorKeysEnabled`, que WebView2 aplica recién
/// en la próxima navegación (cuando se crea esta función la página ya empezó a cargar), y, para que
/// valga desde ya, `IsBrowserAcceleratorKeyEnabled = false` en cada tecla que llega al controlador.
#[cfg(windows)]
fn sin_teclas_del_navegador(ventana: &WebviewWindow) {
    use webview2_com::AcceleratorKeyPressedEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2AcceleratorKeyPressedEventArgs2, ICoreWebView2Settings3,
    };
    use windows_core::Interface;

    let resultado = ventana.with_webview(|webview| {
        let controlador = webview.controller();
        // SAFETY: llamadas COM a un controlador de WebView2 vivo, en el hilo de la interfaz
        // (with_webview corre la closure ahí).
        unsafe {
            match controlador
                .CoreWebView2()
                .and_then(|w| w.Settings())
                .and_then(|s| s.cast::<ICoreWebView2Settings3>())
            {
                Ok(ajustes) => {
                    if let Err(e) = ajustes.SetAreBrowserAcceleratorKeysEnabled(false) {
                        tracing::warn!("no se pudieron desactivar las teclas del navegador: {e}");
                    }
                }
                Err(e) => tracing::warn!("WebView2 sin ICoreWebView2Settings3: {e}"),
            }
            let manejador = AcceleratorKeyPressedEventHandler::create(Box::new(|_, args| {
                if let Some(args) = args {
                    if let Ok(args) = args.cast::<ICoreWebView2AcceleratorKeyPressedEventArgs2>() {
                        args.SetIsBrowserAcceleratorKeyEnabled(false)?;
                    }
                }
                Ok(())
            }));
            let mut token = 0i64;
            if let Err(e) = controlador.add_AcceleratorKeyPressed(&manejador, &mut token) {
                tracing::warn!("no se pudo escuchar las teclas del navegador: {e}");
            }
        }
    });
    if let Err(e) = resultado {
        tracing::warn!("no se pudo ajustar el webview: {e}");
    }
}
