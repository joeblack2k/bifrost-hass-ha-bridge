use std::collections::HashMap;

use hue::error::{HueError, HueResult};
use hue::legacy_api::ApiResourceLink;

use crate::error::ApiResult;
use crate::model::state::LegacyResourceLink;
use crate::resource::Resources;

fn to_api(link: LegacyResourceLink) -> ApiResourceLink {
    ApiResourceLink {
        link_type: link.link_type,
        name: link.name,
        description: link.description,
        classid: link.classid,
        owner: link.owner,
        recycle: link.recycle,
        links: link.links,
    }
}

fn from_api(link: ApiResourceLink) -> LegacyResourceLink {
    LegacyResourceLink {
        link_type: link.link_type,
        name: link.name,
        description: link.description,
        classid: link.classid,
        owner: link.owner,
        recycle: link.recycle,
        links: link.links,
    }
}

pub fn get(res: &Resources) -> HashMap<u32, ApiResourceLink> {
    res.legacy_resource_links()
        .into_iter()
        .map(|(id, link)| (id, to_api(link)))
        .collect()
}

pub fn get_one(res: &Resources, id: u32) -> HueResult<ApiResourceLink> {
    res.legacy_resource_link(id)
        .map(to_api)
        .ok_or(HueError::V1NotFound(id))
}

pub fn create(res: &mut Resources, link: ApiResourceLink) -> ApiResult<u32> {
    let id = res.next_legacy_resource_link_id()?;
    res.put_legacy_resource_link(id, from_api(link));
    Ok(id)
}

pub fn update(res: &mut Resources, id: u32, link: ApiResourceLink) -> ApiResult<()> {
    if res.legacy_resource_link(id).is_none() {
        return Err(HueError::V1NotFound(id).into());
    }
    res.put_legacy_resource_link(id, from_api(link));
    Ok(())
}

pub fn delete(res: &mut Resources, id: u32) -> ApiResult<()> {
    if !res.delete_legacy_resource_link(id) {
        return Err(HueError::V1NotFound(id).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{create, get_one};
    use crate::model::state::State;
    use hue::legacy_api::ApiResourceLink;
    use hue::version::SwVersion;

    #[test]
    fn resource_links_round_trip_through_persistent_store() {
        let mut resources = crate::resource::Resources::new(SwVersion::default(), State::new());
        let id = create(
            &mut resources,
            ApiResourceLink {
                link_type: "Link".to_string(),
                name: "Test lights".to_string(),
                description: "test".to_string(),
                classid: 1,
                owner: Uuid::nil(),
                recycle: false,
                links: vec!["/lights/1".to_string()],
            },
        )
        .expect("resource-link ID");

        let stored = get_one(&resources, id).expect("resource link");
        assert_eq!(id, 1);
        assert_eq!(stored.name, "Test lights");
        assert_eq!(stored.links, vec!["/lights/1"]);
    }
}
