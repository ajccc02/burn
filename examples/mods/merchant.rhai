let merchant_exists = false;
for merchant in query("merchant") {
    if is_demo_merchant(merchant.props) { merchant_exists = true; }
}
if !merchant_exists {
    spawn("merchant", #{
        demo: "merchant",
        name: "Wandering Merchant",
        x: 0,
        gold: 0,
        stock: 100,
        trades: 0
    });
}

// Add a nearby customer if the world lacks a villager equipped to trade.
let customer_exists = false;
for customer in query("villager") {
    if is_customer(customer.props) && customer.props.x >= -1 &&
        customer.props.x <= 4 && customer.props.gold > 0 &&
        customer.props.bread < 9223372036854775807 {
        customer_exists = true;
    }
}
if !customer_exists {
    spawn("villager", #{name: "Market Customer", x: 2, gold: 100, bread: 0});
}

on("tick", "merchant_tick");

fn is_demo_merchant(props) {
    props.contains("demo") && props.demo == "merchant"
}

fn is_customer(props) {
    props.contains("x") && type_of(props.x) == "i64" &&
        props.contains("gold") && type_of(props.gold) == "i64" &&
        props.contains("bread") && type_of(props.bread) == "i64"
}

fn merchant_tick() {
    for merchant in query("merchant") {
        if is_demo_merchant(merchant.props) {
            let x = (merchant.props.x + 1) % 4;
            set(merchant.id, "x", x);
            if x == 0 {
                emit("merchant_patrol", #{merchant: merchant.id});
            }
            if merchant.props.stock > 0 {
                for customer in query("villager") {
                    if is_customer(customer.props) &&
                        customer.props.x >= x - 1 && customer.props.x <= x + 1 &&
                        customer.props.gold > 0 && customer.props.bread < 9223372036854775807 {
                        set(customer.id, "gold", customer.props.gold - 1);
                        set(customer.id, "bread", customer.props.bread + 1);
                        set(merchant.id, "gold", merchant.props.gold + 1);
                        set(merchant.id, "stock", merchant.props.stock - 1);
                        set(merchant.id, "trades", merchant.props.trades + 1);
                        emit("trade", #{merchant: merchant.id, customer: customer.id, item: "bread"});
                        break;
                    }
                }
            }
        }
    }
}
