//! Eased scrolling. A terminal moves in whole lines, so "smooth" here means the
//! view glides toward its target over a few frames instead of jumping there.

use std::time::Instant;

/// Time constant of the glide: about 95% of the way after three times this.
const TAU: f32 = 0.07;

pub struct Smooth {
    pos: f32,
    goal: Option<f32>,
    last: Instant,
}

impl Smooth {
    pub fn new() -> Self {
        Self { pos: 0.0, goal: None, last: Instant::now() }
    }

    pub fn active(&self) -> bool {
        self.goal.is_some()
    }

    pub fn goal(&self) -> Option<usize> {
        self.goal.map(|g| g as usize)
    }

    /// Glide toward `goal`; `current` is what is on screen now (the start when idle).
    pub fn aim(&mut self, current: usize, goal: usize) {
        if self.goal.is_none() {
            self.pos = current as f32;
            self.last = Instant::now();
        }
        self.goal = Some(goal as f32);
    }

    /// Stop gliding (something else set the position directly).
    pub fn stop(&mut self) {
        self.goal = None;
    }

    #[cfg(test)]
    /// Jump to the goal now.
    pub fn finish(&mut self) -> Option<usize> {
        let g = self.goal.take()?;
        self.pos = g;
        Some(g as usize)
    }

    /// Advance to `now`. The value to show, or `None` when there is nothing to animate.
    pub fn step(&mut self, now: Instant) -> Option<usize> {
        let goal = self.goal?;
        let dt = now.saturating_duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        self.pos += (goal - self.pos) * (1.0 - (-dt / TAU).exp());
        if (goal - self.pos).abs() < 0.5 {
            self.goal = None;
            self.pos = goal;
            return Some(goal as usize);
        }
        Some(self.pos.round().max(0.0) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn glides_monotonically_and_lands_exactly() {
        let mut s = Smooth::new();
        s.aim(0, 40);
        assert!(s.active());
        let mut t = Instant::now();
        let mut last = 0;
        let mut frames = 0;
        while let Some(v) = {
            t += Duration::from_millis(16);
            s.step(t)
        } {
            assert!(v >= last && v <= 40, "went backwards or overshot: {last} -> {v}");
            last = v;
            frames += 1;
            assert!(frames < 200, "never settled");
        }
        assert_eq!(last, 40);
        assert!(frames > 5, "should take several frames, took {frames}");
        assert!(!s.active());
    }

    #[test]
    fn retargeting_keeps_the_motion() {
        let mut s = Smooth::new();
        s.aim(10, 20);
        let t = Instant::now() + Duration::from_millis(50);
        let mid = s.step(t).unwrap();
        assert!(mid > 10 && mid < 20);
        // A second wheel notch moves the goal; the position carries on from where it is.
        s.aim(mid, 23);
        assert_eq!(s.goal(), Some(23));
        assert_eq!(s.finish(), Some(23));
    }
}
