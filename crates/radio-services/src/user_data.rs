use crate::{
    Services,
    model::{OsuInstallation, UserData},
};
use anyhow::{Context, Result};

#[derive(Debug)]
pub struct UserDataOverview {
    pub id: i32,
    pub osu_folders: Vec<OsuInstallation>,
}

pub struct UserDataService<'a> {
    pub(crate) services: &'a Services,
}
impl UserDataService<'_> {
    pub async fn get(&self) -> Result<UserData> {
        self.services
            .database
            .user_data()
            .get()
            .await
            .context("Failed to load the stored user data")
    }
    pub async fn overview(&self) -> Result<UserDataOverview> {
        Ok(UserDataOverview {
            id: self.get().await?.id,
            osu_folders: self.services.osu_installations().all().await?,
        })
    }
    pub(crate) async fn lock(
        repository: radio_db::repositories::UserDataRepository<'_>,
    ) -> Result<()> {
        repository.lock().await
    }
}
