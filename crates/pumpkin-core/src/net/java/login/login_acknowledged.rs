#[allow(clippy::wildcard_imports)]
use super::*;

impl PendingConnection {
    pub async fn handle_login_acknowledged(
        &mut self,
        server: &Server,
    ) -> Option<PacketHandlerResult> {
        debug!("Handling login acknowledgement");
        // Only valid once `CLoginSuccess` went out. Without this the claimed
        // profile from `SLoginStart` reaches the config phase, and from there the
        // play phase, without ever being authenticated.
        if !self.login_success_sent {
            self.kick(TextComponent::text("Unexpected login acknowledgement"))
                .await;
            return Some(PacketHandlerResult::Stop);
        }
        self.connection_state.store(ConnectionState::Config);
        self.send_packet_now(&server.get_branding()).await;

        if server.advanced_config.server_links.enabled {
            let mut links: Vec<Link> = Vec::new();
            let kiyo_url = "https://kiyomc.com";

            fn sanitize_link<'a>(link: &'a str, fallback: &'a str) -> &'a str {
                if link.to_ascii_lowercase().contains("pumpkin") {
                    fallback
                } else {
                    link
                }
            }

            let bug_report = sanitize_link(&server.advanced_config.server_links.bug_report, kiyo_url);
            if !bug_report.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::BugReport), bug_report));
            }

            let support = sanitize_link(&server.advanced_config.server_links.support, kiyo_url);
            if !support.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Support), support));
            }

            let status = sanitize_link(&server.advanced_config.server_links.status, kiyo_url);
            if !status.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Status), status));
            }

            let feedback = sanitize_link(&server.advanced_config.server_links.feedback, kiyo_url);
            if !feedback.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Feedback), feedback));
            }

            let community = sanitize_link(&server.advanced_config.server_links.community, kiyo_url);
            if !community.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Community), community));
            }

            let website = sanitize_link(&server.advanced_config.server_links.website, kiyo_url);
            if !website.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Website), website));
            }

            let forums = sanitize_link(&server.advanced_config.server_links.forums, kiyo_url);
            if !forums.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Forums), forums));
            }

            let news = sanitize_link(&server.advanced_config.server_links.news, kiyo_url);
            if !news.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::News), news));
            }

            let announcements = sanitize_link(&server.advanced_config.server_links.announcements, kiyo_url);
            if !announcements.is_empty() {
                links.push(Link::new(
                    Label::BuiltIn(LinkType::Announcements),
                    announcements,
                ));
            }

            for (key, value) in &server.advanced_config.server_links.custom {
                links.push(Link::new(
                    Label::TextComponent(TextComponent::text(key.clone()).into()),
                    sanitize_link(value, kiyo_url),
                ));
            }

            self.send_packet_now(&CConfigServerLinks::new(&links)).await;
        }

        let resource_config = &server.advanced_config.resource_pack.java;
        if resource_config.enabled {
            let uuid = Uuid::new_v3(&uuid::Uuid::NAMESPACE_DNS, resource_config.url.as_bytes());
            let resource_pack = CConfigAddResourcePack::new(
                &uuid,
                &resource_config.url,
                &resource_config.sha1,
                resource_config.force,
                if resource_config.prompt_message.is_empty() {
                    None
                } else {
                    Some(TextComponent::text(resource_config.prompt_message.clone()))
                },
            );

            self.send_packet_now(&resource_pack).await;
        } else {
            self.send_known_packs(server).await;
        }
        debug!("login acknowledged");
        None
    }

    pub async fn send_known_packs(&mut self, server: &Server) {
        let features = server.get_enabled_features();
        self.send_packet_now(&CFeatureFlags::new(&features)).await;
        let version_str = CURRENT_MC_VERSION.to_string();
        let loaded_packs = server.datapack_manager.get_loaded_packs();
        let known_packs = server.get_known_packs(&version_str, &loaded_packs);
        self.send_packet_now(&CKnownPacks::new(&known_packs)).await;
    }
}
