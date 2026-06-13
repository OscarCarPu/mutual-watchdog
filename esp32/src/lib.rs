#![cfg_attr(not(test), no_std)]

pub const ALERT_ACTIVE: u32 = 0xA1E2_7001;

pub enum AlertAction {
    SendAlert,
    SendRecovery,
    NoAction,
}

pub fn compute_alert_action(success: bool, state: &mut u32) -> AlertAction {
    if success {
        if *state == ALERT_ACTIVE {
            *state = 0;
            AlertAction::SendRecovery
        } else {
            AlertAction::NoAction
        }
    } else if *state != ALERT_ACTIVE {
        *state = ALERT_ACTIVE;
        AlertAction::SendAlert
    } else {
        AlertAction::NoAction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_ok_clears_active_alert_and_signals_recovery() {
        let mut state = ALERT_ACTIVE;
        let action = compute_alert_action(true, &mut state);
        assert_eq!(state, 0);
        assert!(matches!(action, AlertAction::SendRecovery));
    }

    #[test]
    fn ping_ok_no_active_alert_no_action() {
        let mut state = 0u32;
        let action = compute_alert_action(true, &mut state);
        assert_eq!(state, 0);
        assert!(matches!(action, AlertAction::NoAction));
    }

    #[test]
    fn ping_err_no_active_alert_sets_active_and_signals_alert() {
        let mut state = 0u32;
        let action = compute_alert_action(false, &mut state);
        assert_eq!(state, ALERT_ACTIVE);
        assert!(matches!(action, AlertAction::SendAlert));
    }

    #[test]
    fn ping_err_alert_already_active_no_action() {
        let mut state = ALERT_ACTIVE;
        let action = compute_alert_action(false, &mut state);
        assert_eq!(state, ALERT_ACTIVE);
        assert!(matches!(action, AlertAction::NoAction));
    }
}
