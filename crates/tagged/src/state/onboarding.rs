use zed::unstable::{
    gpui::{AppContext as _, Entity},
    ui::{Context, Window},
    workspace::Workspace,
};

use crate::{
    scenes::onboarding_item::OnboardingItem,
    state::{profile::Profile, space::Space},
};

/// State tracking a user's first interaction with the app
///
/// Initially this includes creating a first Profile and Space
///
/// There may be future needs like explaining the UI
#[non_exhaustive]
pub struct Onboarding {
    pub profile: Option<Profile>,
    pub space: Option<Space>,

    pub onboarding_item: Option<Entity<OnboardingItem>>,
    workspace: Entity<Workspace>,
}

impl Onboarding {
    pub fn new(workspace: Entity<Workspace>, _cx: &mut Context<Self>) -> Self {
        //
        Self {
            //
            profile: Default::default(),
            space: Default::default(),
            onboarding_item: Default::default(),
            workspace,
        }
    }

    pub fn open_tab(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.workspace.update(cx, |workspace, cx| {
            // If active -> no action
            let active_onboarding = workspace.active_item_as::<OnboardingItem>(cx);
            if let Some(_onboarding_item) = active_onboarding {
                //
                return;
            }

            // If open -> activate
            let open_onboarding = workspace
                .items(cx)
                .find_map(|it| it.downcast::<OnboardingItem>());
            if let Some(onboarding_item) = open_onboarding {
                workspace.activate_item(&onboarding_item, true, true, window, cx);
                return;
            }

            // Otherwise -> Create and activate
            let onboarding_item = cx.new(|cx| OnboardingItem::new(window, cx));
            workspace.add_item_to_active_pane(
                //
                Box::new(onboarding_item),
                Some(0),
                true,
                window,
                cx,
            );
        });
    }
}
