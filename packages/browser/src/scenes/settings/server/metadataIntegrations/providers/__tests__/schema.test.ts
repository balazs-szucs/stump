import { MetadataProvider } from '@stump/graphql'

import { providerRequiresToken } from '../constants'
import { createConfig, getCommonDefaults } from '../schema'

describe('providerRequiresToken', () => {
	it('should not require a token for Open Library', () => {
		expect(providerRequiresToken(MetadataProvider.OpenLibrary)).toBe(false)
	})

	it('should require a token for credentialed providers', () => {
		expect(providerRequiresToken(MetadataProvider.Hardcover)).toBe(true)
		expect(providerRequiresToken(MetadataProvider.ComicVine)).toBe(true)
	})
})

describe('createConfig', () => {
	it('should accept Open Library without an API token', () => {
		const result = createConfig.safeParse({
			providerType: MetadataProvider.OpenLibrary,
			...getCommonDefaults(),
		})

		expect(result.success).toBe(true)
	})

	it('should reject credentialed providers without an API token', () => {
		const result = createConfig.safeParse({
			providerType: MetadataProvider.Hardcover,
			...getCommonDefaults(),
		})

		expect(result.success).toBe(false)
		if (!result.success) {
			expect(result.error.issues.some((issue) => issue.path.includes('apiToken'))).toBe(true)
		}
	})
})
