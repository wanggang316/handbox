import type { PageLoad } from './$types';

export const load: PageLoad = async () => {
  return {
    status: 302,
    redirect: '/settings/account'
  } as unknown as any;
};


