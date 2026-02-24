import { actionService } from '../services/ActionService';
import * as fileActions from './fileActions';
import * as archiveActions from './archiveActions';
import * as driveActions from './driveActions';

export const initializeActions = () => {
    const allActions = {
        ...fileActions,
        ...archiveActions,
        ...driveActions
    };

    Object.values(allActions).forEach(action => {
        // Ensure it looks like an action definition before registering
        if (action && typeof action === 'object' && 'id' in action && 'handler' in action) {
            actionService.register(action as any);
        }
    });
    // console.log(`Initialized ${actionService.getAll().length} actions.`);
};

export * from './fileActions';
export * from './archiveActions';
export * from './driveActions';
