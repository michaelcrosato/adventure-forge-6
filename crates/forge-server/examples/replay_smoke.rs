//! Trusted verification driver, not a blind-player interface or game solver.
//! Execute a reviewed public-action recipe and export only its safe trace.

use forge_content::parse_and_compile_production;
use forge_server::{ActionRequest, ServiceLimits, SessionService, StartRequest};
use std::error::Error;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn Error>> {
    let content = Arc::new(parse_and_compile_production(include_str!(
        "../../../content/split-tide.json"
    ))?);
    let limits = ServiceLimits::default();
    let mut service = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )?;
    let recipe = [
        ("travel_adjacent", Some("lowsail.docks")),
        ("docks.ring_warning", None),
        ("docks.rig_towline", None),
        ("levee.relay_warning", None),
        ("levee.culvert_path", None),
        ("floor.open_relief", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.divert_relief", None),
        ("world.enter_aftermath", None),
        ("return.move_inland", None),
    ];
    for (index, (definition, destination)) in recipe.into_iter().enumerate() {
        let before = service.observe()?;
        let mut page = before.catalog.clone();
        let action_id = loop {
            if let Some(action) = page.actions.iter().find(|action| {
                action.definition_id == definition
                    && destination.is_none_or(|target| {
                        action.parameters.get("destination").map(String::as_str) == Some(target)
                    })
            }) {
                break action.action_id.clone();
            }
            let offset = page.next_offset.ok_or("reviewed action is unavailable")?;
            page = service.catalog(
                &before.observation.state_id,
                offset,
                limits.default_page_size,
            )?;
        };
        let request = ActionRequest {
            command_id: format!("process-smoke-{index}"),
            expected_revision: before.revision,
            expected_state_id: before.observation.state_id,
            action_id,
        };
        let accepted = service.act(request.clone())?;
        if service.act(request)? != accepted {
            return Err("retry changed the acknowledged view".into());
        }
        if index == 2 {
            let checkpoint = service.save()?;
            service = SessionService::resume(content.clone(), &checkpoint, limits.clone())?;
            if service.observe()? != accepted {
                return Err("resuming changed the accepted view".into());
            }
        }
    }
    if service.observe()?.revision != 10 {
        return Err("reviewed process smoke has the wrong action count".into());
    }
    println!("{}", service.save()?);
    Ok(())
}
