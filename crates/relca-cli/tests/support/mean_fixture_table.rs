// Closed wire, independent of production reducers and DTO decoding.
#[allow(dead_code)]
pub const MEAN_FIXTURES: &[&str] = &include!("../../../relca-core/tests/support/mean_names.rs");
fn mean_schema(sampled: bool) -> Ty {
    use Ty::*;
    let value = if sampled {
        Obj(vec![("point",Null(Box::new(MetricNumber))), ("observations",Int), ("censored",Int),
            ("reason",Null(Box::new(Lit(&["aborted","no_samples","truncated","undetermined","not_configured","zero_denominator","beyond_reached_mass"]))))])
    } else {
        Obj(vec![
            ("lower",Null(Box::new(MetricNumber))), ("upper",Null(Box::new(MetricNumber))),
            ("basis",Null(Box::new(Lit(&["exact","per_case_relaxation"])))),
            ("reason",Null(Box::new(Lit(&["aborted","truncated","undetermined","not_configured",
                "zero_denominator","denominator_may_be_zero","beyond_reached_mass"])))),
        ])
    };
    Obj(vec![
        ("name",Str), ("measure",Lit(&["elapsed","work","named_people_work","cost"])), ("property",Null(Box::new(Str))),
        ("form",Lit(&["all_arrivals","reached_only"])),
        ("value",value), ("lower_bound",Null(Box::new(MetricNumber))),
    ])
}
fn validate_means<'a>(q:&'a Value,sampled:bool,names:&std::collections::BTreeSet<&str>,
    metrics:&mut std::collections::BTreeSet<&'a str>) -> Result<(),String> {
    let checked=q["checked"].as_array().unwrap(); let unchecked=q["unchecked"].as_array().unwrap();
    let Some(rows)=q.get("means") else {
        if checked.iter().chain(unchecked).any(|x|x=="means") {return Err("undeclared mean inventory".into());}
        return Ok(());
    };
    let rows=rows.as_array().unwrap();
    if rows.is_empty() {return Err("empty means".into());}
    for row in rows {
        if row["name"].as_str().unwrap().is_empty() || !metrics.insert(row["name"].as_str().unwrap())
            || !(row["property"].is_null() && row["measure"]!="elapsed" && row["form"]=="all_arrivals"
                || row["property"].as_str().is_some_and(|p|names.contains(p))) {return Err("mean identity/property".into());}
        let v=&row["value"]; let reason=&v["reason"]; let lb=&row["lower_bound"];
        if sampled {
            let n = v["observations"].as_u64().ok_or("mean observations")?;
            let c = v["censored"].as_u64().ok_or("mean censored")?;
            if !lb.is_null() || v["point"].is_null() == reason.is_null() || reason.is_null() && (c != 0 || n == 0)
                || matches!(reason.as_str(),Some("aborted"|"no_samples"|"zero_denominator")) && (n != 0 || c != 0)
                || matches!(reason.as_str(),Some("truncated"|"undetermined")) && c == 0
                || reason == "zero_denominator" && row["form"] != "reached_only"
                || reason == "beyond_reached_mass" && (row["form"] != "all_arrivals" || c != 0)
            {return Err("mean sampled point/reason".into());}
            continue;
        }
        let missing=v["lower"].is_null();
        if missing!=v["upper"].is_null() || missing!=v["basis"].is_null() || missing==reason.is_null()
            || !missing && (!lb.is_null() || !metric_le(&v["lower"],&v["upper"])?
                || (v["basis"]=="exact")!=(v["lower"]==v["upper"])) {return Err("mean interval/reason".into());}
        if !lb.is_null() && (row["form"]=="reached_only"
            || !matches!(reason.as_str(),Some("truncated"|"undetermined"|"beyond_reached_mass"))) {return Err("spurious mean bound".into());}
        if row["form"]=="all_arrivals" && matches!(reason.as_str(),Some("zero_denominator"|"denominator_may_be_zero"))
            || row["form"]=="reached_only" && reason=="beyond_reached_mass" {return Err("mean reason/form".into());}
    }
    let aborted=rows.iter().all(|r| matches!(r["value"]["reason"].as_str(),Some("aborted"|"no_samples")));
    if checked.iter().filter(|x|*x=="means").count()!=usize::from(!aborted)
        || unchecked.iter().filter(|x|*x=="means").count()!=usize::from(aborted) {return Err("mean inventory".into());}
    Ok(())
}
