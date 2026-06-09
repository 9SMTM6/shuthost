import type { OperationFailure } from '../../helpers/appStore';
import type { AnyComponent } from '../../helpers/utils';
import { NotifyDurationButton } from './NotifyDurationButton';
import { NotifyOperationFailedButton } from './NotifyOperationFailedButton';
import { NotifyUnscheduledButton } from './NotifyUnscheduledButton';

export const NotificationSection = ((props: {
    hostname: string;
    status: string | undefined;
    operationFailure: OperationFailure | undefined;
}) => (
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
        <div class="flex justify-center">
            <NotifyUnscheduledButton hostname={props.hostname} />
        </div>
        <div class="flex justify-center lg:order-3">
            <NotifyOperationFailedButton hostname={props.hostname} />
        </div>
        <div class="flex justify-center sm:col-span-2 lg:col-span-1 lg:order-2">
            <NotifyDurationButton
                hostname={props.hostname}
                isOnline={props.status === 'online'}
            />
        </div>
    </div>
)) satisfies AnyComponent;
