let volcano_exists = false;
for volcano in query("volcano") {
    if is_demo_volcano(volcano.props) { volcano_exists = true; }
}
if !volcano_exists {
    spawn("volcano", #{demo: "volcano", name: "Mount Ember", eruptions: 0});
}

set_timer(60, "erupt");

fn is_demo_volcano(props) {
    props.contains("demo") && props.demo == "volcano"
}

fn erupt() {
    for volcano in query("volcano") {
        if is_demo_volcano(volcano.props) {
            set(volcano.id, "eruptions", volcano.props.eruptions + 1);
            emit("eruption", #{volcano: volcano.id});
            log("Mount Ember erupted.");
        }
    }
    set_timer(60, "erupt");
}
