use kfnetlist_extract::{
    db::*,
    electrical::{self, MarkerCell, MarkerPort, PortMapping},
    parser::{self, Filters},
    ports::{self, CheckOptions},
    Port, PortTransform,
};
#[test]
fn physical_position_tolerance_is_strict_and_snapping_is_explicit(
) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let make = |x| Port {
        name: String::new(),
        port_type: "optical".into(),
        transform: PortTransform::Physical(
            DCplxTrans::new(1.0, 0.0, false, DVector::new(x, 0.0).unwrap()).unwrap(),
        ),
        layer: LayerInfo::new(1, 0),
        width: 500,
        dbu: 0.001,
        cross_section: 0,
    };
    let a = make(0.0);
    let b = make(0.0001);
    assert_eq!(
        ports::check_connection(&a, &b, CheckOptions::default())? & ports::POSITION,
        0
    );
    assert_ne!(
        ports::check_connection(
            &a,
            &b,
            CheckOptions {
                snapped: true,
                ..Default::default()
            }
        )? & ports::POSITION,
        0
    );
    Ok(())
}
#[test]
fn electrical_marker_copy_preserves_source_and_parser_top(
) -> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let mut layout = Layout::new()?;
    let root = layout.create_cell("WIRE")?;
    let info = LayerInfo::new(1, 0).with_name("M1");
    let layer = layout.layer(&info)?;
    layout.cell_mut(root)?.insert_polygon(
        layer,
        &Polygon::from_box(rlayout::db::Box::new(0, 0, 100, 100))?,
    )?;
    let cells = [MarkerCell {
        name: "WIRE".into(),
        factory: None,
        ports: vec![MarkerPort {
            name: "A".into(),
            port_type: "electrical".into(),
            layer: info.clone(),
            transform: Trans::new(Rotation::R0, false, Vector::new(50, 50)),
        }],
    }];
    let mut extraction = electrical::l2n_elec(
        &layout,
        root,
        &cells,
        &["electrical".into()],
        &[vec![info]],
        &PortMapping::new(),
    )?;
    // Marker extraction reads a duplicate, so the original can still be edited.
    layout.create_cell("UNLOCKED")?;
    let parsed = parser::parse_l2n(&mut extraction, false, &Filters::default())?;
    let parsed = serde_json::to_value(parsed)?;
    assert_eq!(parsed["top_circuit"], "WIRE");
    assert_eq!(
        extraction
            .netlist()?
            .unwrap()
            .circuit_by_name("WIRE")?
            .unwrap()
            .nets()?[0]
            .name()?,
        "A"
    );
    Ok(())
}
