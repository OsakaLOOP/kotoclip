use crate::{
    model::{Register, RegisterRouting},
    prepare::has_long_dialogue,
};

pub fn select_register(text: &str, requested: Register) -> RegisterRouting {
    if requested == Register::Cwj && has_long_dialogue(text) {
        RegisterRouting {
            requested,
            selected: Register::Csj,
            reason: Some("long_dialogue".into()),
        }
    } else {
        RegisterRouting {
            requested,
            selected: requested,
            reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_dialogue_switches_written_request_to_spoken() {
        let routing = select_register("「これはとても長い会話文を含んでいます」", Register::Cwj);
        assert_eq!(routing.selected, Register::Csj);
        assert_eq!(routing.reason.as_deref(), Some("long_dialogue"));
    }
}
