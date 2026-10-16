use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub enum TrayAction {
    Open,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    open_id: MenuId,
    quit_id: MenuId,
}

impl Tray {
    pub fn install() -> Result<Self, String> {
        let open = MenuItem::new("Abrir", true, None);
        let quit = MenuItem::new("Salir", true, None);
        let open_id = open.id().clone();
        let quit_id = quit.id().clone();
        let menu = Menu::new();
        menu.append(&open)
            .map_err(|error| format!("no se pudo crear el menú de la bandeja: {error}"))?;
        menu.append(&quit)
            .map_err(|error| format!("no se pudo crear el menú de la bandeja: {error}"))?;
        let bytes = if crate::brand::taskbar_is_light() {
            crate::brand::TRAY_FOR_LIGHT_BAR
        } else {
            crate::brand::TRAY_FOR_DARK_BAR
        };
        let (width, height, pixels) = crate::brand::rgba(bytes);
        let image = Icon::from_rgba(pixels, width, height)
            .map_err(|error| format!("no se pudo crear el icono: {error}"))?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Keyboard Macros OS")
            .with_icon(image)
            .build()
            .map_err(|error| format!("no se pudo instalar el icono de bandeja: {error}"))?;
        Ok(Self {
            _icon: icon,
            open_id,
            quit_id,
        })
    }

    pub fn poll(&self) -> Option<TrayAction> {
        let mut action = None;
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                action = Some(TrayAction::Open);
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.open_id {
                action = Some(TrayAction::Open);
            } else if event.id == self.quit_id {
                action = Some(TrayAction::Quit);
            }
        }
        action
    }
}
