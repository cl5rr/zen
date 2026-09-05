use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct SpringParams {
    pub damping: f64,
    pub mass: f64,
    pub stiffness: f64,
    pub epsilon: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub from: f64,
    pub to: f64,
    pub initial_velocity: f64,
    pub params: SpringParams,
}

impl SpringParams {
    pub fn new(damping_ratio: f64, stiffness: f64, epsilon: f64) -> Self {
        let damping_ratio = damping_ratio.max(0.);
        let stiffness = stiffness.max(0.);
        let epsilon = epsilon.max(0.);

        let mass = 1.;
        let critical_damping = 2. * (mass * stiffness).sqrt();
        let damping = damping_ratio * critical_damping;

        Self {
            damping,
            mass,
            stiffness,
            epsilon,
        }
    }
}

impl Spring {
    pub fn value_at(&self, t: Duration) -> f64 {
        self.oscillate(t.as_secs_f64())
    }

    pub fn velocity_at(&self, t: Duration) -> f64 {
        self.oscillate_velocity(t.as_secs_f64())
    }

    pub fn duration(&self) -> Duration {
        const DELTA: f64 = 0.001;

        let beta = self.params.damping / (2. * self.params.mass);

        if beta.abs() <= f64::EPSILON || beta < 0. {
            return Duration::MAX;
        }

        if (self.to - self.from).abs() <= f64::EPSILON {
            return Duration::ZERO;
        }

        let omega0 = (self.params.stiffness / self.params.mass).sqrt();

        let mut x0 = -self.params.epsilon.ln() / beta;

        if (beta - omega0).abs() <= f64::from(f32::EPSILON) || beta < omega0 {
            return Duration::from_secs_f64(x0);
        }

        let mut y0 = self.oscillate(x0);
        let m = (self.oscillate(x0 + DELTA) - y0) / DELTA;

        let mut x1 = (self.to - y0 + m * x0) / m;
        let mut y1 = self.oscillate(x1);

        let mut i = 0;
        while (self.to - y1).abs() > self.params.epsilon {
            if i > 1000 {
                return Duration::ZERO;
            }

            x0 = x1;
            y0 = y1;

            let m = (self.oscillate(x0 + DELTA) - y0) / DELTA;

            x1 = (self.to - y0 + m * x0) / m;
            y1 = self.oscillate(x1);

            if !y1.is_finite() {
                return Duration::from_secs_f64(x0);
            }

            i += 1;
        }

        Duration::from_secs_f64(x1)
    }

    pub fn clamped_duration(&self) -> Option<Duration> {
        let beta = self.params.damping / (2. * self.params.mass);

        if beta.abs() <= f64::EPSILON || beta < 0. {
            return Some(Duration::MAX);
        }

        if (self.to - self.from).abs() <= f64::EPSILON {
            return Some(Duration::ZERO);
        }

        let mut i = 1u16;
        let mut y = self.oscillate(f64::from(i) / 1000.);

        while (self.to - self.from > f64::EPSILON && self.to - y > self.params.epsilon)
            || (self.from - self.to > f64::EPSILON && y - self.to > self.params.epsilon)
        {
            if i > 3000 {
                return None;
            }

            i += 1;
            y = self.oscillate(f64::from(i) / 1000.);
        }

        Some(Duration::from_millis(u64::from(i)))
    }

    fn oscillate_velocity(&self, t: f64) -> f64 {
        let b = self.params.damping;
        let m = self.params.mass;
        let k = self.params.stiffness;
        let v0 = self.initial_velocity;

        let beta = b / (2. * m);
        let omega0 = (k / m).sqrt();

        let x0 = self.from - self.to;
        let c = beta * x0 + v0;

        let envelope = (-beta * t).exp();

        if (beta - omega0).abs() <= f64::from(f32::EPSILON) {
            let g = x0 + c * t;
            envelope * (c - beta * g)
        } else if beta < omega0 {
            let omega1 = ((omega0 * omega0) - (beta * beta)).sqrt();
            let (sin, cos) = (omega1 * t).sin_cos();
            let g = x0 * cos + (c / omega1) * sin;
            let dg = -x0 * omega1 * sin + c * cos;
            envelope * (dg - beta * g)
        } else {
            let omega2 = ((beta * beta) - (omega0 * omega0)).sqrt();
            let sinh = (omega2 * t).sinh();
            let cosh = (omega2 * t).cosh();
            let g = x0 * cosh + (c / omega2) * sinh;
            let dg = x0 * omega2 * sinh + c * cosh;
            envelope * (dg - beta * g)
        }
    }

    fn oscillate(&self, t: f64) -> f64 {
        let b = self.params.damping;
        let m = self.params.mass;
        let k = self.params.stiffness;
        let v0 = self.initial_velocity;

        let beta = b / (2. * m);
        let omega0 = (k / m).sqrt();

        let x0 = self.from - self.to;

        let envelope = (-beta * t).exp();

        if (beta - omega0).abs() <= f64::from(f32::EPSILON) {
            self.to + envelope * (x0 + (beta * x0 + v0) * t)
        } else if beta < omega0 {
            let omega1 = ((omega0 * omega0) - (beta * beta)).sqrt();

            self.to
                + envelope
                    * (x0 * (omega1 * t).cos() + ((beta * x0 + v0) / omega1) * (omega1 * t).sin())
        } else {
            let omega2 = ((beta * beta) - (omega0 * omega0)).sqrt();

            self.to
                + envelope
                    * (x0 * (omega2 * t).cosh() + ((beta * x0 + v0) / omega2) * (omega2 * t).sinh())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn springs() -> [(&'static str, Spring); 3] {
        [
            (
                "underdamped",
                Spring {
                    from: 0.,
                    to: 100.,
                    initial_velocity: 37.,
                    params: SpringParams::new(0.6, 800., 0.0001),
                },
            ),
            (
                "critically damped",
                Spring {
                    from: 0.,
                    to: 100.,
                    initial_velocity: 37.,
                    params: SpringParams::new(1.0, 800., 0.0001),
                },
            ),
            (
                "overdamped",
                Spring {
                    from: 0.,
                    to: 100.,
                    initial_velocity: 37.,
                    params: SpringParams::new(1.8, 800., 0.0001),
                },
            ),
        ]
    }

    #[test]
    fn velocity_at_zero_is_initial_velocity() {
        for (name, spring) in springs() {
            let v = spring.velocity_at(Duration::ZERO);
            assert!(
                (v - spring.initial_velocity).abs() < 1e-6,
                "{name}: velocity_at(0) = {v}, expected {}",
                spring.initial_velocity
            );
        }
    }

    #[test]
    fn analytic_velocity_matches_numerical_derivative() {
        const H: f64 = 1e-6;

        for (name, spring) in springs() {
            for step in 1..60 {
                let t = f64::from(step) * 0.005;
                let lo = t - H;
                let hi = t + H;

                let analytic = spring.velocity_at(Duration::from_secs_f64(t));
                let numerical = (spring.value_at(Duration::from_secs_f64(hi))
                    - spring.value_at(Duration::from_secs_f64(lo)))
                    / (hi - lo);

                let tol = 1e-3 * analytic.abs().max(1.0);
                assert!(
                    (analytic - numerical).abs() < tol,
                    "{name} at t={t}: analytic {analytic}, numerical {numerical}"
                );
            }
        }
    }

    #[test]
    fn settled_spring_has_no_velocity() {
        let spring = Spring {
            from: 100.,
            to: 100.,
            initial_velocity: 0.,
            params: SpringParams::new(1.0, 800., 0.0001),
        };
        for step in 0..10 {
            let t = Duration::from_secs_f64(f64::from(step) * 0.05);
            assert!(spring.velocity_at(t).abs() < 1e-9);
        }
    }

    #[test]
    fn overdamped_spring_equal_from_to_nan() {
        let spring = Spring {
            from: 0.,
            to: 0.,
            initial_velocity: 0.,
            params: SpringParams::new(1.15, 850., 0.0001),
        };
        let _ = spring.duration();
        let _ = spring.clamped_duration();
        let _ = spring.value_at(Duration::ZERO);
    }

    #[test]
    fn overdamped_spring_duration_panic() {
        let spring = Spring {
            from: 0.,
            to: 1.,
            initial_velocity: 0.,
            params: SpringParams::new(6., 1200., 0.0001),
        };
        let _ = spring.duration();
        let _ = spring.clamped_duration();
        let _ = spring.value_at(Duration::ZERO);
    }
}
