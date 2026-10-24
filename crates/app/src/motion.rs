// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The critically damped spring that slides the sidebar.
//!
//! The panel is animated as one fraction between closed and open. The fraction
//! follows a critically damped spring toward its target, evaluated in closed
//! form, so the motion is smooth at any frame rate, can be retargeted
//! mid-slide from wherever it is without a jump, and settles in about a
//! quarter of a second.
//!
//! A reader who asks the system to reduce motion gets the target immediately
//! instead of the slide.

/// The angular frequency of the spring, in radians per second.
///
/// Higher is stiffer. At 35 the spring covers most of its distance in the
/// first tenth of a second and arrives within the settle bound in about a
/// quarter of a second.
const OMEGA: f64 = 35.0;

/// The largest step the spring advances in one call, in seconds.
///
/// A stalled frame advances by this bound instead of the real elapsed time, so
/// a gap between frames cannot throw the panel across the screen.
const MAX_STEP: f64 = 0.05;

/// The distance from the target below which the spring has arrived.
const SETTLE: f64 = 0.001;

/// The speed below which a spring near its target counts as arrived.
const VELOCITY_SETTLE: f64 = 0.05;

/// Whether the reader has asked the system to reduce motion.
pub fn reduce_motion() -> bool {
    #[cfg(target_os = "macos")]
    {
        objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// One animated fraction between zero and one, driven toward a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// The animated value.
    value: f64,
    /// The first derivative of the value.
    velocity: f64,
    /// The value the spring is driven toward.
    target: f64,
}

impl Spring {
    /// A spring at rest on a value.
    pub const fn at_rest(value: f64) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
        }
    }

    /// Drives the spring toward a new target, keeping its position and speed.
    ///
    /// Retargeting mid-slide is what makes reversing the panel smooth: the
    /// slide turns around from where it is instead of restarting.
    pub fn retarget(&mut self, target: f64) {
        self.target = target;
    }

    /// Stops the spring on a value at once.
    pub fn snap_to(&mut self, value: f64) {
        self.value = value;
        self.velocity = 0.0;
        self.target = value;
    }

    /// The animated value.
    pub fn value(self) -> f64 {
        self.value
    }

    /// The value the spring is heading for.
    pub fn target(self) -> f64 {
        self.target
    }

    /// Whether the spring is still away from its target.
    pub fn moving(self) -> bool {
        (self.value - self.target).abs() > SETTLE || self.velocity.abs() > VELOCITY_SETTLE
    }

    /// Advances the spring by an elapsed time in seconds.
    ///
    /// The step is bounded, so a caller may pass whatever the frame clock
    /// claims without checking it first.
    pub fn advance(&mut self, seconds: f64) {
        let step = seconds.clamp(0.0, MAX_STEP);
        // The closed form of x'' + 2w x' + w^2 (x - target) = 0 for the
        // critically damped case: the value lands on the target with no
        // oscillation, which is the feel a panel slide wants.
        let away = self.value - self.target;
        let slope = self.velocity + OMEGA * away;
        let decay = (-OMEGA * step).exp();
        self.value = self.target + (away + slope * step) * decay;
        self.velocity = (slope - OMEGA * (away + slope * step)) * decay;
        if !self.moving() {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame step the tests advance by.
    const FRAME: f64 = 1.0 / 60.0;

    #[test]
    fn a_spring_settles_on_its_target_without_overshooting() {
        let mut spring = Spring::at_rest(0.0);
        spring.retarget(1.0);
        let mut steps = 0;
        while spring.moving() {
            spring.advance(FRAME);
            steps += 1;
            assert!(steps < 60, "the spring did not settle within a second");
            assert!(
                (0.0..=1.0).contains(&spring.value()),
                "the spring left the range it animates: {}",
                spring.value()
            );
        }
        assert!(
            spring.value() > 0.999,
            "the spring stopped short of its target: {}",
            spring.value()
        );
        // The closed form covers most of the distance long before it settles.
        assert!(steps < 30, "the spring took {steps} frames to settle");
    }

    #[test]
    fn retargeting_mid_slide_keeps_the_value_continuous() {
        let mut spring = Spring::at_rest(0.0);
        spring.retarget(1.0);
        for _ in 0..4 {
            spring.advance(FRAME);
        }
        let before = spring.value();
        spring.retarget(0.0);
        spring.advance(0.0);
        assert_eq!(spring.value(), before, "retargeting moved the value");
        spring.retarget(1.0);
        spring.advance(0.0);
        assert_eq!(spring.value(), before, "retargeting back moved the value");
    }

    #[test]
    fn a_stalled_frame_advances_by_a_bounded_step() {
        let mut spring = Spring::at_rest(0.0);
        spring.retarget(1.0);
        spring.advance(3600.0);
        assert!(
            spring.value() < 0.99,
            "one stalled call completed the slide: {}",
            spring.value()
        );
        assert!(
            spring.value() > 0.5,
            "one stalled call barely moved the slide: {}",
            spring.value()
        );
    }

    #[test]
    fn snapping_lands_the_spring_on_a_value() {
        let mut spring = Spring::at_rest(0.0);
        spring.retarget(1.0);
        spring.advance(FRAME);
        spring.snap_to(1.0);
        assert!(!spring.moving());
        assert_eq!(spring.value(), 1.0);
    }
}
