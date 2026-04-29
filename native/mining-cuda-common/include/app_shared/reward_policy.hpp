#pragma once

namespace app {

enum class RewardKind {
    Invite,
    Balance,
};

struct RewardPolicy {
    RewardKind kind;
    const char* product_name;
    const char* reward_name;
    const char* preference;
    bool reset_client_after_success;
};

inline const RewardPolicy& invite_reward_policy() {
    static const RewardPolicy policy{
        .kind = RewardKind::Invite,
        .product_name = "mining-invite-cuda",
        .reward_name = "邀请码",
        .preference = "invite",
        .reset_client_after_success = true,
    };
    return policy;
}

inline const RewardPolicy& balance_reward_policy() {
    static const RewardPolicy policy{
        .kind = RewardKind::Balance,
        .product_name = "mining-balance-cuda",
        .reward_name = "余额兑换码",
        .preference = "balance",
        .reset_client_after_success = true,
    };
    return policy;
}

} // namespace app
