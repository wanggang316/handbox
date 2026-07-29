
import { apiCall } from './index';
import type {
	Provider,
	AddProviderRequest,
	ProviderConfig,
	ProviderConfigsResponse,
	UUID
} from '../types';

export async function getProviders(): Promise<Provider[]> {
  return apiCall<Provider[]>('provider_list');
}

export async function getProvider(providerId: UUID): Promise<Provider> {
  return apiCall<Provider>('provider_get', { providerId: providerId });
}

export async function createProvider(config: AddProviderRequest): Promise<Provider> {
  return apiCall<Provider>('provider_create', { config });
}

export async function updateProvider(
  providerId: UUID,
  config: Partial<AddProviderRequest>
): Promise<Provider> {
  return apiCall<Provider>('provider_update', { providerId: providerId, config });
}

export async function deleteProvider(providerId: UUID): Promise<void> {
  return apiCall<void>('provider_delete', { providerId: providerId });
}

export async function toggleProvider(providerId: UUID, enabled: boolean): Promise<Provider> {
	return apiCall<Provider>('provider_toggle', {
		request: {
			provider_id: providerId,
			enabled
		}
	});
}

/** Provider config templates for the add-provider flow. */
export async function getProviderConfigs(): Promise<ProviderConfigsResponse> {
  return apiCall<ProviderConfigsResponse>('get_provider_configs');
}

export async function getProviderConfigByType(
	providerType: string
): Promise<ProviderConfig | null> {
	return apiCall<ProviderConfig | null>('get_provider_config_by_type', {
		provider_type: providerType
	});
}

