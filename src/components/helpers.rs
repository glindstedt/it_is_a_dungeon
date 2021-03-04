use super::{GivenName, Name};

pub fn named(name: Option<&Name>, given_name: Option<&GivenName>) -> String {
    if let Some(name) = name {
        if let Some(given_name) = given_name {
            format!("{} the {}", given_name.name, name.name)
        } else {
            format!("Unnamed {}", name.name)
        }
    } else {
        "Unknown".into()
    }
}
