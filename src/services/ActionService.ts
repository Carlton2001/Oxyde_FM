import { ActionDefinition, ActionContext } from '../types/actions';

export class ActionService {
    private static instance: ActionService;
    private actions: Map<string, ActionDefinition> = new Map();

    private constructor() {
        // Private constructor for Singleton
    }

    public static getInstance(): ActionService {
        if (!ActionService.instance) {
            ActionService.instance = new ActionService();
        }
        return ActionService.instance;
    }

    /**
     * Register a new action or overwrite an existing one
     */
    public register(action: ActionDefinition): void {
        if (this.actions.has(action.id)) {
            console.warn(`ActionService: Overwriting existing action with id '${action.id}'`);
        }
        this.actions.set(action.id, action);
    }

    /**
     * Unregister an action by ID
     */
    public unregister(id: string): boolean {
        return this.actions.delete(id);
    }

    /**
     * Get an action definition by ID
     */
    public get(id: string): ActionDefinition | undefined {
        return this.actions.get(id);
    }

    /**
     * Get all registered actions
     */
    public getAll(): ActionDefinition[] {
        return Array.from(this.actions.values());
    }

    /**
     * Execute an action if it exists and is enabled
     */
    public async execute(id: string, context: ActionContext): Promise<void> {
        const action = this.get(id);
        if (!action) {
            console.error(`ActionService: Action '${id}' not found`);
            return;
        }

        if (action.isEnabled && !action.isEnabled(context)) {
            console.warn(`ActionService: Action '${id}' is currently disabled`);
            return;
        }

        try {
            await action.handler(context);
        } catch (error) {
            console.error(`ActionService: Error executing action '${id}':`, error);
            throw error;
        }
    }

    /**
     * Helper to check if an action should be visible in the UI
     */
    public isVisible(id: string, context: ActionContext): boolean {
        const action = this.get(id);
        if (!action) return false;
        return action.isVisible ? action.isVisible(context) : true;
    }
}

export const actionService = ActionService.getInstance();
