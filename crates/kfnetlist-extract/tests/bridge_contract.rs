use kfnetlist_extract::connected_geometry;
use rlayout::db::*;

#[test]
fn extraction_leases_source_and_retains_graph() -> Result<(), rlayout::Error> {
    let mut layout = Layout::new()?;
    let root = layout.create_cell("WIRE")?;
    let layer = layout.layer(&LayerInfo::new(1, 0))?;
    let polygon = Polygon::from_box(rlayout::db::Box::new(0, 0, 100, 100))?;
    layout.cell_mut(root)?.insert_polygon(layer, &polygon)?;
    let extraction = connected_geometry(&layout, root, &[vec![layer]])?;
    assert!(layout
        .cell_mut(root)?
        .insert_polygon(layer, &polygon)
        .is_err());
    let graph = extraction.netlist()?.unwrap();
    drop(extraction);
    drop(layout);
    assert_eq!(graph.circuit_by_name("WIRE")?.unwrap().nets()?.len(), 1);
    Ok(())
}

#[test]
fn rejects_foreign_and_stale_cells() -> Result<(), rlayout::Error> {
    let mut a = Layout::new()?;
    let mut b = Layout::new()?;
    let root = a.create_cell("ROOT")?;
    b.create_cell("ROOT")?;
    assert!(connected_geometry(&b, root, &[]).is_err());
    a.delete_cell(root)?;
    assert!(connected_geometry(&a, root, &[]).is_err());
    Ok(())
}
