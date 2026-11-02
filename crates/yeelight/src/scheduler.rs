use crate::protocol::Command;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Power,
    Brightness,
    Color,
}

#[derive(Clone, Debug)]
struct Pending {
    command: Command,
    not_before: Instant,
}

pub struct Scheduler {
    pending: HashMap<Channel, Pending>,
    last_sent: HashMap<Channel, Instant>,
    sent_at: VecDeque<Instant>,
    quota_paused_until: Option<Instant>,
    quota_problem_since: Option<Instant>,
    pub min_interval: Duration,
    pub max_per_minute: usize,
    pub stuck_after: Duration,
    pub quota_pause: Duration,
}

#[derive(Debug)]
pub enum PollAction {
    Send(Channel, Command),
    Wait(Duration),
    Idle,
}

impl Scheduler {
    pub fn new(min_interval: Duration, max_per_minute: usize) -> Self {
        Self {
            pending: HashMap::new(),
            last_sent: HashMap::new(),
            sent_at: VecDeque::new(),
            quota_paused_until: None,
            quota_problem_since: None,
            min_interval,
            max_per_minute,
            stuck_after: Duration::from_secs(120),
            quota_pause: Duration::from_secs(15),
        }
    }

    pub fn push(&mut self, channel: Channel, command: Command, now: Instant) {
        let spacing = match channel {
            Channel::Power => Duration::ZERO,
            Channel::Brightness | Channel::Color => self.min_interval,
        };
        let earliest = self
            .last_sent
            .get(&channel)
            .map(|sent| *sent + spacing)
            .unwrap_or(now);
        let mut not_before = earliest.max(now);
        if let Some(existing) = self.pending.get(&channel) {
            not_before = not_before.max(existing.not_before);
        }
        self.pending.insert(
            channel,
            Pending {
                command,
                not_before,
            },
        );
    }

    /// Put a failed command back unless a newer value is already waiting.
    pub fn requeue(&mut self, channel: Channel, command: Command, now: Instant) {
        self.pending.entry(channel).or_insert(Pending {
            command,
            not_before: now,
        });
    }

    pub fn note_sent(&mut self, now: Instant) {
        self.sent_at.push_back(now);
    }

    pub fn on_success(&mut self) {
        self.quota_problem_since = None;
        self.quota_paused_until = None;
    }

    pub fn on_quota_exceeded(&mut self, now: Instant) {
        if self.quota_problem_since.is_none() {
            self.quota_problem_since = Some(now);
        }
        self.quota_paused_until = Some(now + self.quota_pause);
    }

    pub fn is_stuck(&self, now: Instant) -> bool {
        self.quota_problem_since
            .is_some_and(|started| now.saturating_duration_since(started) >= self.stuck_after)
    }

    pub fn poll(&mut self, now: Instant) -> PollAction {
        let window = Duration::from_secs(60);
        self.sent_at
            .retain(|sent| now.saturating_duration_since(*sent) < window);

        if let Some(until) = self.quota_paused_until {
            if now < until {
                return PollAction::Wait(until.saturating_duration_since(now));
            }
        }

        if self.sent_at.len() >= self.max_per_minute {
            if let Some(oldest) = self.sent_at.front() {
                let ready_at = *oldest + window;
                if now < ready_at {
                    return PollAction::Wait(ready_at.saturating_duration_since(now));
                }
            }
        }

        let due = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.not_before <= now)
            .min_by_key(|(channel, _)| priority(**channel))
            .map(|(channel, _)| *channel);

        if let Some(channel) = due {
            let pending = self.pending.remove(&channel).expect("channel is pending");
            self.last_sent.insert(channel, now);
            self.sent_at.push_back(now);
            return PollAction::Send(channel, pending.command);
        }

        if let Some(wait) = self
            .pending
            .values()
            .map(|pending| pending.not_before.saturating_duration_since(now))
            .min()
        {
            return PollAction::Wait(wait);
        }
        PollAction::Idle
    }
}

fn priority(channel: Channel) -> u8 {
    match channel {
        Channel::Power => 0,
        Channel::Brightness => 1,
        Channel::Color => 2,
    }
}

pub fn channel_of(command: &Command) -> Option<Channel> {
    match command {
        Command::SetPower { .. } => Some(Channel::Power),
        Command::SetBright { .. } => Some(Channel::Brightness),
        Command::SetRgb { .. } | Command::SetHsv { .. } | Command::SetCt { .. } => {
            Some(Channel::Color)
        }
        Command::GetProp { .. } | Command::SetName { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bright(value: u8) -> Command {
        Command::set_bright(value)
    }

    #[tokio::test(start_paused = true)]
    async fn slider_keeps_only_the_latest_value() {
        let mut scheduler = Scheduler::new(Duration::from_millis(250), 50);
        let now = Instant::now();
        scheduler.push(Channel::Brightness, bright(10), now);
        scheduler.push(Channel::Brightness, bright(40), now);
        scheduler.push(Channel::Brightness, bright(70), now);
        match scheduler.poll(now) {
            PollAction::Send(_, Command::SetBright { brightness, .. }) => {
                assert_eq!(brightness, 70)
            }
            other => panic!("expected the coalesced value, got {other:?}"),
        }
        scheduler.push(Channel::Brightness, bright(15), Instant::now());
        assert!(matches!(
            scheduler.poll(Instant::now()),
            PollAction::Wait(_)
        ));
        tokio::time::advance(Duration::from_millis(250)).await;
        match scheduler.poll(Instant::now()) {
            PollAction::Send(_, Command::SetBright { brightness, .. }) => {
                assert_eq!(brightness, 15)
            }
            other => panic!("expected the trailing value, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn quota_window_holds_the_fifty_first_command() {
        let mut scheduler = Scheduler::new(Duration::from_millis(250), 50);
        for index in 0..50 {
            let now = Instant::now();
            scheduler.push(Channel::Power, Command::set_power(index % 2 == 0), now);
            assert!(
                matches!(scheduler.poll(now), PollAction::Send(_, _)),
                "command {index} should be sent"
            );
        }
        let now = Instant::now();
        scheduler.push(Channel::Power, Command::set_power(true), now);
        assert!(matches!(scheduler.poll(now), PollAction::Wait(_)));
        tokio::time::advance(Duration::from_secs(60)).await;
        assert!(matches!(
            scheduler.poll(Instant::now()),
            PollAction::Send(_, _)
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn quota_errors_become_stuck_after_the_threshold() {
        let mut scheduler = Scheduler::new(Duration::from_millis(250), 50);
        scheduler.stuck_after = Duration::from_millis(200);
        scheduler.quota_pause = Duration::from_millis(50);
        let start = Instant::now();
        scheduler.on_quota_exceeded(start);
        assert!(!scheduler.is_stuck(start));
        tokio::time::advance(Duration::from_millis(200)).await;
        assert!(scheduler.is_stuck(Instant::now()));
        scheduler.on_success();
        assert!(!scheduler.is_stuck(Instant::now()));
    }

    #[tokio::test(start_paused = true)]
    async fn requeue_does_not_replace_a_newer_value() {
        let mut scheduler = Scheduler::new(Duration::from_millis(250), 50);
        let now = Instant::now();
        scheduler.requeue(Channel::Brightness, bright(10), now);
        scheduler.push(Channel::Brightness, bright(80), now);
        scheduler.requeue(Channel::Brightness, bright(10), now);
        match scheduler.poll(now) {
            PollAction::Send(_, Command::SetBright { brightness, .. }) => {
                assert_eq!(brightness, 80)
            }
            other => panic!("{other:?}"),
        }
    }
}
