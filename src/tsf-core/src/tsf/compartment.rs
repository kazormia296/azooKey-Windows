use windows::{
    core::{Interface, Result, GUID},
    Win32::UI::TextServices::{
        ITfCompartment, ITfCompartmentEventSink, ITfCompartmentEventSink_Impl, ITfCompartmentMgr,
        ITfSource, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
    },
};

use super::text_service::TextService_Impl;

#[derive(Clone)]
pub struct CompartmentEntry {
    pub compartment: ITfCompartment,
    pub cookie: u32,
}

pub const WATCHED_COMPARTMENTS: &[GUID] = &[GUID_COMPARTMENT_KEYBOARD_OPENCLOSE];

pub fn advise_compartment_sink(
    mgr: &ITfCompartmentMgr,
    sink: &ITfCompartmentEventSink,
    guid: &GUID,
) -> Result<CompartmentEntry> {
    unsafe {
        let compartment = mgr.GetCompartment(guid)?;
        let source: ITfSource = compartment.cast()?;
        let cookie = source.AdviseSink(&ITfCompartmentEventSink::IID, sink)?;
        Ok(CompartmentEntry {
            compartment,
            cookie,
        })
    }
}

pub fn unadvise_compartment_sink(entry: &CompartmentEntry) {
    unsafe {
        let source = entry.compartment.cast::<ITfSource>().ok();
        if let Some(source) = source {
            let _ = source.UnadviseSink(entry.cookie);
        }
    }
}

pub fn clear_compartment(mgr: &ITfCompartmentMgr, tid: u32, guid: &GUID) -> Result<()> {
    unsafe { mgr.ClearCompartment(tid, guid) }
}

impl ITfCompartmentEventSink_Impl for TextService_Impl {
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        let guid = unsafe { *rguid };
        tracing::debug!("Compartment changed: {:?}", guid);
        let compartments = self.compartments.borrow();
        match guid {
            GUID_COMPARTMENT_KEYBOARD_OPENCLOSE => {
                if let Some(_entry) = compartments.get(&guid) {
                    tracing::debug!("Keyboard open/close compartment changed");
                }
            }
            _ => {
                tracing::debug!("Unknown compartment changed: {:?}", guid);
            }
        }
        Ok(())
    }
}
