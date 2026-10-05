//! Asking one resource of its source: the runner's own sources, then the
//! data seam (LLP 1030 D7, LLP 1039 D1, LLP 1038 D5).

use super::{lines, DataError, DataSource, Runner, RunnerError};
use crate::request::Answer;
use exact_plan::Value;

impl<D: DataSource> Runner<D> {
    pub(super) fn query(&mut self, i: usize, args: &[Value]) -> Result<Answer, RunnerError> {
        let row = &self.plan.resources[i];
        let source = self.plan.str(row.source).to_string();
        let resource = self.plan.str(row.name).to_string();
        // Delivery is the runner's own (LLP 1030 D7): the data seam never
        // sees it, and a data crate could not answer it if it did.
        if source == crate::delivery::SOURCE {
            return self
                .delivery_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        // @ref LLP 1039 D1 — host facts never reach the app data source.
        if source == crate::viewport::SOURCE {
            return self
                .viewport_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        if source == crate::time::SOURCE {
            return self
                .time_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        if source == crate::page::SOURCE {
            return self
                .page_answer(i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        if source == crate::surface_record::SOURCE {
            let answer = self.links.surface_answer.ok_or_else(|| RunnerError::Data {
                resource: resource.clone(),
                error: DataError::Unavailable("this host links no surfaces".into()),
            })?;
            return answer(&self.plan, &self.surface_records, i)
                .map(Answer::Now)
                .map_err(|error| RunnerError::Data { resource, error });
        }
        // @ref LLP 1038 D5 / §8 — distinguish asked sources from compiled boot values.
        self.log(lines::query(&resource, &source));
        let answer =
            self.data
                .answer_for(super::Target::Resource(i), &mut self.store, &source, args);
        for line in self.data.take_logs() {
            self.log(line);
        }
        answer.map_err(|error| RunnerError::Data { resource, error })
    }
}
