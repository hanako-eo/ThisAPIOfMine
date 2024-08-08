#[cfg(test)]
use mockall::automock;
use octocrab::models::repos::Release;
use octocrab::Octocrab;

use crate::errors::{InternalError, Result};
use crate::game_data::Repo;

#[cfg_attr(test, automock(type Pager = Vec<Release>;))]
pub trait RepoFetcher {
    type Pager: IntoIterator<Item = Release>;

    async fn get_releases(&self, repo: &Repo) -> Result<Self::Pager>;
    async fn get_last_release(&self, repo: &Repo) -> Result<Release>;
}

impl RepoFetcher for Octocrab {
    type Pager = std::vec::IntoIter<Release>;

    async fn get_releases(&self, repo: &Repo) -> Result<<Self as RepoFetcher>::Pager> {
        Ok(self
            .repos(repo.owner(), repo.repository())
            .releases()
            .list()
            .send()
            .await?
            .into_iter())
    }

    async fn get_last_release(&self, repo: &Repo) -> Result<Release> {
        self.repos(repo.owner(), repo.repository())
            .releases()
            .get_latest()
            .await
            .map_err(|err| InternalError::External(Box::new(err)))
    }
}
