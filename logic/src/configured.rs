//! A concrete default constructor selected by an app's generated host entry.

/// Give an app's host a Default data type without retaining runtime policy switches.
#[macro_export]
macro_rules! configured {
    ($name:ident,$data:ty,$factory:expr) => {
        struct $name($crate::Swappable<$data>);
        impl Default for $name {
            fn default() -> Self {
                Self(($factory)())
            }
        }
        impl $crate::exact_runner::DataSource for $name {
            fn app_id(&self) -> &str {
                $crate::exact_runner::DataSource::app_id(&self.0)
            }
            fn grants(&self) -> &str {
                $crate::exact_runner::DataSource::grants(&self.0)
            }
            fn revision(&self) -> Option<&str> {
                $crate::exact_runner::DataSource::revision(&self.0)
            }
            fn ready(&self) -> bool {
                $crate::exact_runner::DataSource::ready(&self.0)
            }
            fn canvas_surfaces(&self) -> Vec<(String, usize)> {
                $crate::exact_runner::DataSource::canvas_surfaces(&self.0)
            }
            fn draw(
                &mut self,
                request: &$crate::exact_runner::DrawRequest<'_>,
                ctx: &$crate::exact_runner::exact_canvas::Context2d,
            ) -> $crate::exact_runner::Drawn {
                $crate::exact_runner::DataSource::draw(&mut self.0, request, ctx)
            }
            fn canvases_retired(&mut self, retired: &[(u64, u32)]) {
                $crate::exact_runner::DataSource::canvases_retired(&mut self.0, retired)
            }
            fn preload(&self) -> Result<bool, $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::preload(&self.0)
            }
            fn configure_storage(
                &mut self,
                data: std::path::PathBuf,
                cache: std::path::PathBuf,
                temporary: std::path::PathBuf,
            ) -> Result<(), $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::configure_storage(
                    &mut self.0,
                    data,
                    cache,
                    temporary,
                )
            }
            fn continuation(
                &mut self,
                token: u64,
            ) -> Option<Box<dyn FnOnce() -> $crate::exact_runner::Outcome + Send>> {
                $crate::exact_runner::DataSource::continuation(&mut self.0, token)
            }
            fn placement(&self) -> $crate::exact_runner::Placement {
                $crate::exact_runner::DataSource::placement(&self.0)
            }
            fn interrupt(&self) -> Option<$crate::exact_runner::Interrupt> {
                $crate::exact_runner::DataSource::interrupt(&self.0)
            }
            fn forgotten(&mut self, in_flight: &[$crate::exact_runner::InFlight<'_>]) {
                $crate::exact_runner::DataSource::forgotten(&mut self.0, in_flight)
            }
            fn dispatch(
                &mut self,
                token: u64,
                store: &$crate::exact_runner::Store,
            ) -> $crate::exact_runner::Dispatch {
                $crate::exact_runner::DataSource::dispatch(&mut self.0, token, store)
            }
            fn release(
                &mut self,
                store: &$crate::exact_runner::Store,
            ) -> Vec<(u64, $crate::exact_runner::Dispatch)> {
                $crate::exact_runner::DataSource::release(&mut self.0, store)
            }
            fn discard(&mut self, token: u64) {
                $crate::exact_runner::DataSource::discard(&mut self.0, token)
            }
            fn bind(&mut self, plan: &$crate::exact_plan::Plan) {
                $crate::exact_runner::DataSource::bind(&mut self.0, plan)
            }
            fn adopt(
                &mut self,
                source: &str,
                args: &[$crate::exact_plan::Value],
                value: &$crate::exact_plan::Value,
            ) {
                $crate::exact_runner::DataSource::adopt(&mut self.0, source, args, value)
            }
            fn activate(&mut self) -> Result<(), $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::activate(&mut self.0)
            }
            fn activate_for_validation(&mut self) -> Result<(), $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::activate_for_validation(&mut self.0)
            }
            fn replacement(
                &self,
                plan: &[u8],
                receipt: &str,
                module: Vec<u8>,
            ) -> Result<Self, $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::replacement(&self.0, plan, receipt, module)
                    .map(Self)
            }
            fn query(
                &mut self,
                source: &str,
                args: &[$crate::exact_plan::Value],
            ) -> Result<$crate::exact_plan::Value, $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::query(&mut self.0, source, args)
            }
            fn answer(
                &mut self,
                store: &mut $crate::exact_runner::Store,
                source: &str,
                args: &[$crate::exact_plan::Value],
            ) -> Result<$crate::exact_runner::Answer, $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::answer(&mut self.0, store, source, args)
            }
            fn parse(
                &mut self,
                store: &mut $crate::exact_runner::Store,
                source: &str,
                args: &[$crate::exact_plan::Value],
                outcome: $crate::exact_runner::Outcome,
            ) -> Result<$crate::exact_runner::Answer, $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::parse(&mut self.0, store, source, args, outcome)
            }
            fn answer_for(
                &mut self,
                target: $crate::exact_runner::Target,
                store: &mut $crate::exact_runner::Store,
                source: &str,
                args: &[$crate::exact_plan::Value],
            ) -> Result<$crate::exact_runner::Answer, $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::answer_for(
                    &mut self.0,
                    target,
                    store,
                    source,
                    args,
                )
            }
            fn parse_for(
                &mut self,
                target: $crate::exact_runner::Target,
                store: &mut $crate::exact_runner::Store,
                source: &str,
                args: &[$crate::exact_plan::Value],
                outcome: $crate::exact_runner::Outcome,
            ) -> Result<$crate::exact_runner::Answer, $crate::exact_runner::DataError> {
                $crate::exact_runner::DataSource::parse_for(
                    &mut self.0,
                    target,
                    store,
                    source,
                    args,
                    outcome,
                )
            }
        }
    };
}
