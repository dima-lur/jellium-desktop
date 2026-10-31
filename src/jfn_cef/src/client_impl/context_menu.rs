use cef::*;
use std::os::raw::c_int;
use std::sync::Arc;

use jfn_platform_abi::LogicalPoint;
use jfn_platform_abi::lang::ContextMenuLabel;

use crate::client::Inner;

wrap_context_menu_handler! {
    pub struct JfnContextMenuHandlerBuilder {
        inner: Arc<Inner>,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            let Some(m) = model else { return };
            let reload_id: c_int = MenuId::RELOAD.get_raw() as c_int;
            if m.index_of(reload_id) < 0 {
                let label = self.inner.app.locale().menu_label(ContextMenuLabel::Reload);
                m.add_item(reload_id, Some(&CefString::from(label.as_str())));
            }
        }
        fn run_context_menu(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
            callback: Option<&mut RunContextMenuCallback>,
        ) -> c_int {
            let (Some(_browser), Some(params), Some(model), Some(callback)) =
                (browser, params, model, callback)
            else {
                return 0;
            };
            let anchor = LogicalPoint {
                x: params.xcoord(),
                // The anchor is in the web overlay's own view space; the app menu
                // is placed in window space, so the strip the overlay was sized
                // below is added back.
                y: params.ycoord() + self.inner.view_top(),
            };
            self.inner.menu.lock().open_context_menu(
                callback.clone(),
                model,
                anchor,
                self.inner.channel.sender(),
            );
            1
        }
    }
}
