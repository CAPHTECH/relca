//! U2 hand inputs and strict selected-field comparator, independent of lowering
//! and diff evaluation. Byte spans are from the authored input tokens, not CLI.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const CONFIGURATIONS: [&str; 3] = ["Left", "Right", "Third"];
pub fn source(per_case: bool, amended: bool) -> String {
    let instance = if per_case { "instance per_case; " } else { "" };
    let rule = if amended {
        "people [p]\n actor Rule { version \"v1\"; decides flag: bool = true }\n authority Grant { basis \"rule procedure\"; grants p to change Rule }"
    } else {
        ""
    };
    let amendment = if amended {
        "amend Rule from \"v1\" to \"v2\" by p under Grant { flag = false }\n intent { change rule Rule reason \"change flag\" }"
    } else {
        ""
    };
    let include = if amended { "Invoice, Rule" } else { "Invoice" };
    format!(
        r#"model InstanceDiffBoundary version "0.2" {{
 actor Invoice {{ {instance}source document }}
 {rule}
 process P {{ end done }}
 process Q {{ end done }}
 cut K {{ question "instance boundary"; include [{include}] }}
 configuration Left {{ cut K; process P }}
 configuration Right {{ cut K; process P; {amendment} }}
 configuration Third {{ cut K; process Q }}
 bounds {{ cases 1; live_tasks 2; clock 10min }}
 verify reachable end.done
}}
"#
    )
}

#[derive(Clone, Copy, Debug)]
pub enum Rejection {
    NestedInstance,
    NestedActor,
    DuplicateActor,
    SharedKeyword,
    DuplicateInstance,
    AmendOrdinary,
    AmendMissingField,
}
pub const REJECTIONS: [Rejection; 7] = [
    Rejection::NestedInstance,
    Rejection::NestedActor,
    Rejection::DuplicateActor,
    Rejection::SharedKeyword,
    Rejection::DuplicateInstance,
    Rejection::AmendOrdinary,
    Rejection::AmendMissingField,
];
impl Rejection {
    pub fn name(self) -> &'static str {
        match self {
            Self::NestedInstance => "nested_instance",
            Self::NestedActor => "nested_actor",
            Self::DuplicateActor => "duplicate_actor",
            Self::SharedKeyword => "shared_keyword",
            Self::DuplicateInstance => "duplicate_instance",
            Self::AmendOrdinary => "amend_ordinary",
            Self::AmendMissingField => "amend_missing_field",
        }
    }
    pub fn source(self) -> String {
        let s = source(true, false);
        match self {
            Self::NestedInstance => s.replace("process P;  }", "process P; instance per_case }"),
            Self::NestedActor => s.replace(
                "process P;  }",
                "process P; actor Invoice { source document } }",
            ),
            Self::DuplicateActor => s.replace(
                " process P {",
                " actor Invoice { source document }\n process P {",
            ),
            Self::SharedKeyword => s.replace("instance per_case", "instance shared"),
            Self::DuplicateInstance => s.replace(
                "instance per_case;",
                "instance per_case; instance per_case;",
            ),
            Self::AmendOrdinary => source(true, true)
                .replace("grants p to change Rule", "grants p to change Invoice")
                .replace("amend Rule from", "amend Invoice from"),
            Self::AmendMissingField => {
                source(true, true).replace("{ flag = false }", "{ instance = per_case }")
            }
        }
    }
    pub fn verdict(self) -> &'static str {
        if matches!(self, Self::AmendOrdinary | Self::AmendMissingField) {
            "invalid"
        } else {
            "unsupported"
        }
    }
    pub fn exit(self) -> i32 {
        if self.verdict() == "invalid" { 3 } else { 4 }
    }
    pub fn diagnostics(self) -> Value {
        let s = self.source();
        let error = |code: &str, message: &str, start: usize, len: usize| json!({"code":code,"severity":"error","message":message,"span":{"start":start,"end":start+len}});
        let token_after = |anchor: &str, token: &str| {
            let start = s.find(anchor).unwrap() + anchor.len();
            start + s[start..].find(token).unwrap()
        };
        let rows = match self {
            Self::NestedInstance => vec![error(
                "U-SYN-0",
                "unsupported syntax: instance",
                token_after("configuration Right", "instance"),
                8,
            )],
            Self::NestedActor => vec![error(
                "U-SYN-0",
                "unsupported syntax: actor",
                token_after("configuration Right", "actor"),
                5,
            )],
            Self::DuplicateActor => vec![error(
                "U-SEM-0",
                "duplicate Actor declaration",
                0,
                s.rfind('}').unwrap() + 1,
            )],
            Self::SharedKeyword => vec![error(
                "U-SYN-0",
                "unsupported syntax: Actor instance",
                s.find("shared").unwrap(),
                6,
            )],
            Self::DuplicateInstance => vec![error(
                "U-SEM-0",
                "duplicate Actor instance",
                token_after("instance per_case;", "instance"),
                "instance per_case".len(),
            )],
            Self::AmendOrdinary => vec![
                error(
                    "E-CON-0",
                    "change grant requires a rule Actor",
                    token_after("grants p to change ", "Invoice"),
                    7,
                ),
                error(
                    "E-CON-0",
                    "amend requires a rule Actor",
                    token_after("amend ", "Invoice"),
                    7,
                ),
            ],
            Self::AmendMissingField => vec![error(
                "E-NAME-0",
                "undefined rule value",
                token_after("under Grant {", "instance"),
                8,
            )],
        };
        Value::Array(rows)
    }
}
fn equal(actual: &Value, wanted: &Value, label: &str) -> Result<(), String> {
    if actual != wanted {
        Err(format!("{label}: wanted {wanted} got {actual}"))
    } else {
        Ok(())
    }
}
fn instance_values(report: &Value) -> Result<Value, String> {
    let rows = report["assumptions"]
        .as_array()
        .ok_or("missing assumptions")?;
    Ok(Value::Array(
        rows.iter()
            .filter(|r| {
                r["field"]
                    .as_str()
                    .is_some_and(|f| f.ends_with(".instance"))
            })
            .map(|r| json!({"field":r["field"],"value":r["value"]}))
            .collect(),
    ))
}
pub fn positive(diff: &Value, per_case: bool, amended: bool, exit: i32) -> Result<(), String> {
    if exit != 0 {
        return Err(format!("exit {exit}"));
    }
    equal(&diff["verdict"], &json!("satisfied"), "root verdict")?;
    let instances = if per_case {
        json!([{"field":"Invoice.instance","value":"per_case"}])
    } else {
        json!([])
    };
    for side in ["from", "to"] {
        let r = &diff[side]["report"];
        equal(&r["verdict"], &json!("satisfied"), "side verdict")?;
        equal(
            &r["bounds"],
            &json!({"cases":1,"live_tasks":2,"clock":10,"truncated":false}),
            "bounds",
        )?;
        equal(
            &r["properties"],
            &json!([{"id":"reachable end.done","verdict":"satisfied"}]),
            "properties",
        )?;
        equal(
            &r["terminals"],
            &json!([{"end":"done","elapsed":{"min":0,"max":0},"work":{"min":0,"max":0},"result_at":null}]),
            "terminal hand value",
        )?;
        equal(&r["diagnostics"], &json!([]), "side diagnostics")?;
        equal(&instance_values(r)?, &instances, "instance map")?;
        equal(&diff["agency"][side], &r["agency"], "Agency copy")?;
    }
    let changes = if amended {
        json!([{"kind":"rule","target":"Rule","change":"modified","reason":"change flag","loses":null,
            "from":{"version":"v1","values":[{"name":"flag","value":true}],"amendment":null},
            "to":{"version":"v2","values":[{"name":"flag","value":false}],"amendment":{"from":"v1","to":"v2","by":"p","under":"Grant","under_version":null}}}])
    } else {
        json!([])
    };
    equal(&diff["differences"], &changes, "configuration changes")?;
    equal(
        &diff["agency"]["differences"]["status"],
        &json!("derived"),
        "Agency diff status",
    )?;
    equal(
        &diff["agency"]["differences"]["rows"],
        &json!([]),
        "Agency changes",
    )?;
    if diff["unchecked"]
        .as_array()
        .ok_or("unchecked")?
        .iter()
        .any(|v| v.as_str().is_some_and(|s| s.contains("instance")))
    {
        return Err("unreachable instance difference must not be invented".into());
    }
    Ok(())
}
pub fn rejected(diff: &Value, case: Rejection, exit: i32) -> Result<(), String> {
    if exit != case.exit() {
        return Err(format!("rejection exit {exit}"));
    }
    equal(
        &diff["verdict"],
        &json!(case.verdict()),
        "rejection verdict",
    )?;
    equal(&diff["diagnostics"], &json!([]), "root diagnostics")?;
    equal(&diff["differences"], &json!([]), "no computed changes")?;
    for side in ["from", "to"] {
        let r = &diff[side]["report"];
        equal(&r["verdict"], &json!(case.verdict()), "side rejection")?;
        equal(&r["diagnostics"], &case.diagnostics(), "exact diagnostics")?;
        equal(&r["agency"], &Value::Null, "no Agency")?;
        equal(&r["properties"], &json!([]), "no properties")?;
        equal(&r["terminals"], &json!([]), "no terminals")?;
        equal(
            &r["bounds"],
            &json!({"cases":null,"live_tasks":null,"clock":null,"truncated":null}),
            "no execution bounds",
        )?;
    }
    Ok(())
}
