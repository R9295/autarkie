pub type Action = usize;
pub type Reward = f64;

/**
 * Defines the Bandit trait.
 * A bandit algorithm aims to optimize the reward produced by **choosing** an action (or arm) and using all
 * the feedback (rewards) available to update (and improve) its selection policy.
 * 
 * A bandit algorithm can:
 *  - **choose** an action (also called *arm*)
 *  - **update** its policy depending on the reward obtained by chosing a given action.
 */
pub trait Bandit {
    
    /**
     * returns the next action to choose
     */
    fn choose(&self) -> Action;

    /**
     * udpates the bandit policy depending on the action taken and the reward obtained
     */
    fn update(&mut self, a: Action, r: Reward);

    /**
     * [OPTIONAL IMPLEMENTATION] returns the bandit name and parameters
     */
    fn str(&self) -> std::string::String { return "TODO".to_string(); }

}


/**
 * implements an update type. Either it is an average over time (stationary) or
 * updates with a constant step size (Nonstationary)
 */
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum UpdateType {
    Average,
    Nonstationary(f64)
}


/**
 * updates the average given the following parameters:
 * - **a** past average reward
 * - **r** current reward
 * - **n** nb trials (last one included)
 */
pub fn update_average(previous:Reward, current:Reward, n:u64) -> Reward {
    return update_step_average(previous, current, 1./(n as f64));
}

/**
 * updates the average given the following parameters:
 * - **a** past average reward
 * - **r** current reward
 * - **u** step size
 */
pub fn update_step_average(previous:Reward, current: Reward, u:f64) -> Reward {
    return previous + (current-previous)*u;
}
